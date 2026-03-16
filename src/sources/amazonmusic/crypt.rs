use aes::Aes128;
use ctr::Ctr128BE;
use tracing::{debug, warn};

type Aes128Ctr = Ctr128BE<Aes128>;

pub struct CencDecryptor {
    key: [u8; 16],
}

impl CencDecryptor {
    pub fn from_hex(hex_key: &str) -> Result<Self, String> {
        let trimmed = hex_key.trim();
        if trimmed.len() != 32 {
            return Err(format!(
                "decryption key must be 32 hex chars, got {}",
                trimmed.len()
            ));
        }
        let mut key = [0u8; 16];
        hex::decode_to_slice(trimmed, &mut key).map_err(|e| format!("invalid hex key: {e}"))?;
        Ok(Self { key })
    }

    pub fn decrypt_buffer(&self, buf: &mut [u8]) -> Result<(), String> {
        patch_moov_headers(buf)?;

        if let Ok(fragments) = locate_fragments(buf) {
            for frag in fragments {
                let _ = self.decrypt_fragment(buf, &frag);
            }
        }

        Ok(())
    }

    fn decrypt_fragment(&self, buf: &mut [u8], frag: &FragmentInfo) -> Result<(), String> {
        let mdat_payload_start = frag.mdat_offset + 8;
        let mdat_payload_end = frag.mdat_offset + frag.mdat_size;

        if mdat_payload_end > buf.len() {
            return Err("mdat extends past buffer".into());
        }

        if frag.sample_ivs.is_empty() {
            debug!("fragment has no sample IVs, skipping decryption");
            return Ok(());
        }

        let mut cursor = mdat_payload_start;
        for (idx, sample) in frag.samples.iter().enumerate() {
            let iv_bytes = frag
                .sample_ivs
                .get(idx)
                .ok_or_else(|| format!("missing IV for sample {idx}"))?;

            let sample_end = cursor + sample.size;
            if sample_end > mdat_payload_end || sample_end > buf.len() {
                warn!("sample {} exceeds mdat boundary, stopping", idx);
                break;
            }

            let mut full_iv = [0u8; 16];
            let copy_len = iv_bytes.len().min(16);
            full_iv[..copy_len].copy_from_slice(&iv_bytes[..copy_len]);

            if !sample.subsamples.is_empty() {
                let mut pos = cursor;
                for sub in &sample.subsamples {
                    pos += sub.clear as usize;
                    let enc_len = sub.encrypted as usize;
                    if pos + enc_len > sample_end {
                        break;
                    }
                    self.ctr_decrypt(&full_iv, &mut buf[pos..pos + enc_len]);
                    pos += enc_len;
                }
            } else {
                self.ctr_decrypt(&full_iv, &mut buf[cursor..sample_end]);
            }

            cursor = sample_end;
        }

        Ok(())
    }

    fn ctr_decrypt(&self, iv: &[u8; 16], data: &mut [u8]) {
        self.ctr_decrypt_with_offset(iv, data, 0);
    }

    fn ctr_decrypt_with_offset(&self, iv: &[u8; 16], data: &mut [u8], offset: usize) {
        use ctr::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
        let mut cipher = Aes128Ctr::new(&self.key.into(), iv.into());
        if offset > 0 {
            cipher.seek(offset as u64);
        }
        cipher.apply_keystream(data);
    }

    pub fn ctr_decrypt_external(&self, iv_bytes: &[u8], data: &mut [u8], offset: usize) {
        let mut full_iv = [0u8; 16];
        let copy_len = iv_bytes.len().min(16);
        full_iv[..copy_len].copy_from_slice(&iv_bytes[..copy_len]);
        self.ctr_decrypt_with_offset(&full_iv, data, offset);
    }
}

pub fn parse_moof_external(moof: &[u8]) -> (Vec<SampleEntry>, u8) {
    parse_moof(moof)
}

pub fn extract_sample_ivs_external(moof: &[u8], iv_size_hint: u8, count: usize) -> Vec<Vec<u8>> {
    extract_sample_ivs(moof, iv_size_hint, count)
}

struct FragmentInfo {
    mdat_offset: usize,
    mdat_size: usize,
    samples: Vec<SampleEntry>,
    sample_ivs: Vec<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct SampleEntry {
    pub size: usize,
    pub subsamples: Vec<SubsampleEntry>,
}

#[derive(Debug, Clone)]
pub struct SubsampleEntry {
    pub clear: u32,
    pub encrypted: u32,
}

fn locate_fragments(buf: &[u8]) -> Result<Vec<FragmentInfo>, String> {
    let mut frags = Vec::new();
    let mut pos = 0;

    while pos + 8 <= buf.len() {
        let box_size =
            u32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        let box_type = &buf[pos + 4..pos + 8];

        if box_size < 8 || pos + box_size > buf.len() {
            break;
        }

        if box_type == b"moof" {
            let moof_end = pos + box_size;
            let mdat_offset = moof_end;

            if mdat_offset + 8 > buf.len() {
                pos += box_size;
                continue;
            }

            let mdat_size_raw = u32::from_be_bytes([
                buf[mdat_offset],
                buf[mdat_offset + 1],
                buf[mdat_offset + 2],
                buf[mdat_offset + 3],
            ]) as usize;
            let mdat_type = &buf[mdat_offset + 4..mdat_offset + 8];

            if mdat_type != b"mdat" || mdat_size_raw < 8 {
                pos += box_size;
                continue;
            }

            let (samples, per_sample_iv_size) = parse_moof(&buf[pos..moof_end]);
            let ivs = extract_sample_ivs(&buf[pos..moof_end], per_sample_iv_size, samples.len());

            frags.push(FragmentInfo {
                mdat_offset,
                mdat_size: mdat_size_raw,
                samples,
                sample_ivs: ivs,
            });

            pos = mdat_offset + mdat_size_raw;
            continue;
        }

        pos += box_size;
    }

    Ok(frags)
}

fn parse_moof(moof: &[u8]) -> (Vec<SampleEntry>, u8) {
    let mut samples = Vec::new();
    let mut default_sample_size: u32 = 0;
    let mut per_sample_iv_size: u8 = 0;
    let mut pos = 8;

    while pos + 8 <= moof.len() {
        let sz =
            u32::from_be_bytes([moof[pos], moof[pos + 1], moof[pos + 2], moof[pos + 3]]) as usize;
        let typ = &moof[pos + 4..pos + 8];

        if sz < 8 || pos + sz > moof.len() {
            break;
        }

        if typ == b"traf" {
            let (s, dsz, iv_sz) = parse_traf(&moof[pos..pos + sz]);
            if !s.is_empty() {
                samples = s;
            }
            if dsz > 0 {
                default_sample_size = dsz;
            }
            if iv_sz > 0 {
                per_sample_iv_size = iv_sz;
            }
        }

        pos += sz;
    }

    if default_sample_size > 0 {
        for s in &mut samples {
            if s.size == 0 {
                s.size = default_sample_size as usize;
            }
        }
    }

    (samples, per_sample_iv_size)
}

fn parse_traf(traf: &[u8]) -> (Vec<SampleEntry>, u32, u8) {
    let mut samples = Vec::new();
    let mut default_sample_size: u32 = 0;
    let mut per_sample_iv_size: u8 = 0;
    let mut pos = 8;

    while pos + 8 <= traf.len() {
        let sz =
            u32::from_be_bytes([traf[pos], traf[pos + 1], traf[pos + 2], traf[pos + 3]]) as usize;
        let typ = &traf[pos + 4..pos + 8];

        if sz < 8 || pos + sz > traf.len() {
            break;
        }

        match typ {
            b"tfhd" => {
                default_sample_size = parse_tfhd_default_size(&traf[pos..pos + sz]);
            }
            b"trun" => {
                samples = parse_trun(&traf[pos..pos + sz]);
            }
            b"senc" => {
                let (ivs, subsubs) = parse_senc(&traf[pos..pos + sz], per_sample_iv_size);
                for (i, (iv_data, subs)) in ivs.iter().zip(subsubs.iter()).enumerate() {
                    if i < samples.len() && !subs.is_empty() {
                        samples[i].subsamples = subs.clone();
                    }
                    let _ = iv_data;
                }
            }
            b"sbgp" | b"sgpd" | b"saiz" | b"saio" => {}
            _ => {}
        }

        pos += sz;
    }

    if per_sample_iv_size == 0 {
        per_sample_iv_size = detect_iv_size_from_senc(traf, samples.len());
    }

    (samples, default_sample_size, per_sample_iv_size)
}

fn parse_tfhd_default_size(tfhd: &[u8]) -> u32 {
    if tfhd.len() < 12 {
        return 0;
    }
    let flags = u32::from_be_bytes([0, tfhd[9], tfhd[10], tfhd[11]]);

    let mut off = 12;
    off += 4;
    if flags & 0x01 != 0 {
        off += 8;
    }
    if flags & 0x02 != 0 {
        off += 4;
    }
    if flags & 0x08 != 0 {
        off += 4;
    }
    if flags & 0x10 != 0 && off + 4 <= tfhd.len() {
        return u32::from_be_bytes([tfhd[off], tfhd[off + 1], tfhd[off + 2], tfhd[off + 3]]);
    }
    0
}

fn parse_trun(trun: &[u8]) -> Vec<SampleEntry> {
    if trun.len() < 12 {
        return Vec::new();
    }

    let flags = u32::from_be_bytes([0, trun[9], trun[10], trun[11]]);
    let sample_count = u32::from_be_bytes([trun[12], trun[13], trun[14], trun[15]]) as usize;

    let mut off = 16;
    if flags & 0x01 != 0 {
        off += 4;
    }
    if flags & 0x04 != 0 {
        off += 4;
    }

    let has_duration = flags & 0x100 != 0;
    let has_size = flags & 0x200 != 0;
    let has_flags = flags & 0x400 != 0;
    let has_cts = flags & 0x800 != 0;

    let mut samples = Vec::with_capacity(sample_count);
    for _ in 0..sample_count {
        if has_duration {
            off += 4;
        }
        let size = if has_size && off + 4 <= trun.len() {
            let s = u32::from_be_bytes([trun[off], trun[off + 1], trun[off + 2], trun[off + 3]]);
            off += 4;
            s as usize
        } else {
            if has_size {
                off += 4;
            }
            0
        };
        if has_flags {
            off += 4;
        }
        if has_cts {
            off += 4;
        }
        samples.push(SampleEntry {
            size,
            subsamples: Vec::new(),
        });
    }

    samples
}

fn parse_senc(senc: &[u8], iv_size_hint: u8) -> (Vec<Vec<u8>>, Vec<Vec<SubsampleEntry>>) {
    if senc.len() < 12 {
        return (Vec::new(), Vec::new());
    }

    let flags = u32::from_be_bytes([0, senc[9], senc[10], senc[11]]);
    let sample_count = u32::from_be_bytes([senc[12], senc[13], senc[14], senc[15]]) as usize;
    let has_subsamples = flags & 0x02 != 0;

    let iv_size = if iv_size_hint > 0 {
        iv_size_hint as usize
    } else {
        8
    };
    let mut off = 16;
    let mut ivs = Vec::with_capacity(sample_count);
    let mut all_subs = Vec::with_capacity(sample_count);

    for _ in 0..sample_count {
        if off + iv_size > senc.len() {
            break;
        }
        ivs.push(senc[off..off + iv_size].to_vec());
        off += iv_size;

        let mut subs = Vec::new();
        if has_subsamples {
            if off + 2 > senc.len() {
                break;
            }
            let sub_count = u16::from_be_bytes([senc[off], senc[off + 1]]) as usize;
            off += 2;
            for _ in 0..sub_count {
                if off + 6 > senc.len() {
                    break;
                }
                let clear = u16::from_be_bytes([senc[off], senc[off + 1]]) as u32;
                let encrypted = u32::from_be_bytes([
                    senc[off + 2],
                    senc[off + 3],
                    senc[off + 4],
                    senc[off + 5],
                ]);
                off += 6;
                subs.push(SubsampleEntry { clear, encrypted });
            }
        }
        all_subs.push(subs);
    }

    (ivs, all_subs)
}

fn detect_iv_size_from_senc(traf: &[u8], sample_count: usize) -> u8 {
    if sample_count == 0 {
        return 8;
    }

    let mut pos = 8;
    while pos + 8 <= traf.len() {
        let sz =
            u32::from_be_bytes([traf[pos], traf[pos + 1], traf[pos + 2], traf[pos + 3]]) as usize;
        let typ = &traf[pos + 4..pos + 8];
        if sz < 8 || pos + sz > traf.len() {
            break;
        }

        if typ == b"senc" && sz >= 16 {
            let flags = u32::from_be_bytes([0, traf[pos + 9], traf[pos + 10], traf[pos + 11]]);
            let payload_after_header = sz - 16;
            let has_sub = flags & 0x02 != 0;

            if !has_sub && sample_count > 0 {
                let candidate = payload_after_header / sample_count;
                if candidate == 8 || candidate == 16 {
                    return candidate as u8;
                }
            }
        }

        pos += sz;
    }

    8
}

fn extract_sample_ivs(moof: &[u8], iv_size_hint: u8, sample_count: usize) -> Vec<Vec<u8>> {
    let mut pos = 8;
    while pos + 8 <= moof.len() {
        let sz =
            u32::from_be_bytes([moof[pos], moof[pos + 1], moof[pos + 2], moof[pos + 3]]) as usize;
        let typ = &moof[pos + 4..pos + 8];
        if sz < 8 || pos + sz > moof.len() {
            break;
        }

        if typ == b"traf" {
            return extract_senc_ivs(&moof[pos..pos + sz], iv_size_hint, sample_count);
        }
        pos += sz;
    }
    Vec::new()
}

fn extract_senc_ivs(traf: &[u8], iv_size_hint: u8, _sample_count: usize) -> Vec<Vec<u8>> {
    let mut pos = 8;
    while pos + 8 <= traf.len() {
        let sz =
            u32::from_be_bytes([traf[pos], traf[pos + 1], traf[pos + 2], traf[pos + 3]]) as usize;
        let typ = &traf[pos + 4..pos + 8];
        if sz < 8 || pos + sz > traf.len() {
            break;
        }

        if typ == b"senc" {
            let (ivs, _) = parse_senc(&traf[pos..pos + sz], iv_size_hint);
            return ivs;
        }

        pos += sz;
    }
    Vec::new()
}

pub fn patch_moov_headers(buf: &mut [u8]) -> Result<(), String> {
    let mut pos = 0;
    while pos + 8 <= buf.len() {
        let box_size =
            u32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]) as usize;
        let box_type = &buf[pos + 4..pos + 8];

        if box_size < 8 || pos + box_size > buf.len() {
            break;
        }

        if box_type == b"moov" {
            patch_moov_box(&mut buf[pos..pos + box_size]);
            return Ok(());
        }

        pos += box_size;
    }

    Ok(())
}

fn patch_moov_box(moov: &mut [u8]) {
    let mut pos = 8;
    while pos + 8 <= moov.len() {
        let sz =
            u32::from_be_bytes([moov[pos], moov[pos + 1], moov[pos + 2], moov[pos + 3]]) as usize;
        if sz < 8 || pos + sz > moov.len() {
            break;
        }
        let typ = [moov[pos + 4], moov[pos + 5], moov[pos + 6], moov[pos + 7]];
        if &typ == b"trak" {
            patch_trak(&mut moov[pos..pos + sz]);
        }
        pos += sz;
    }
}

fn patch_trak(trak: &mut [u8]) {
    let mut pos = 8;
    while pos + 8 <= trak.len() {
        let sz =
            u32::from_be_bytes([trak[pos], trak[pos + 1], trak[pos + 2], trak[pos + 3]]) as usize;
        if sz < 8 || pos + sz > trak.len() {
            break;
        }
        let typ = [trak[pos + 4], trak[pos + 5], trak[pos + 6], trak[pos + 7]];
        if &typ == b"mdia" {
            patch_mdia(&mut trak[pos..pos + sz]);
        }
        pos += sz;
    }
}

fn patch_mdia(mdia: &mut [u8]) {
    let mut pos = 8;
    while pos + 8 <= mdia.len() {
        let sz =
            u32::from_be_bytes([mdia[pos], mdia[pos + 1], mdia[pos + 2], mdia[pos + 3]]) as usize;
        if sz < 8 || pos + sz > mdia.len() {
            break;
        }
        let typ = [mdia[pos + 4], mdia[pos + 5], mdia[pos + 6], mdia[pos + 7]];
        if &typ == b"minf" {
            patch_minf(&mut mdia[pos..pos + sz]);
        }
        pos += sz;
    }
}

fn patch_minf(minf: &mut [u8]) {
    let mut pos = 8;
    while pos + 8 <= minf.len() {
        let sz =
            u32::from_be_bytes([minf[pos], minf[pos + 1], minf[pos + 2], minf[pos + 3]]) as usize;
        if sz < 8 || pos + sz > minf.len() {
            break;
        }
        let typ = [minf[pos + 4], minf[pos + 5], minf[pos + 6], minf[pos + 7]];
        if &typ == b"stbl" {
            patch_stbl(&mut minf[pos..pos + sz]);
        }
        pos += sz;
    }
}

fn patch_stbl(stbl: &mut [u8]) {
    let mut pos = 8;
    while pos + 8 <= stbl.len() {
        let sz =
            u32::from_be_bytes([stbl[pos], stbl[pos + 1], stbl[pos + 2], stbl[pos + 3]]) as usize;
        if sz < 8 || pos + sz > stbl.len() {
            break;
        }
        let typ = [stbl[pos + 4], stbl[pos + 5], stbl[pos + 6], stbl[pos + 7]];
        if &typ == b"stsd" {
            patch_stsd(&mut stbl[pos..pos + sz]);
        }
        pos += sz;
    }
}

fn patch_stsd(stsd: &mut [u8]) {
    if stsd.len() < 16 {
        return;
    }

    stsd[12] = 0;
    stsd[13] = 0;
    stsd[14] = 0;
    stsd[15] = 1;

    let mut entry_pos = 16;
    while entry_pos + 8 <= stsd.len() {
        let entry_sz = u32::from_be_bytes([
            stsd[entry_pos],
            stsd[entry_pos + 1],
            stsd[entry_pos + 2],
            stsd[entry_pos + 3],
        ]) as usize;

        if entry_sz < 8 || entry_pos + entry_sz > stsd.len() {
            break;
        }

        let codec_tag = [
            stsd[entry_pos + 4],
            stsd[entry_pos + 5],
            stsd[entry_pos + 6],
            stsd[entry_pos + 7],
        ];

        if &codec_tag == b"enca" {
            let original = find_original_codec(&stsd[entry_pos..entry_pos + entry_sz]);
            let replacement = original.unwrap_or(*b"mp4a");
            debug!(
                "patching stsd entry: enca -> {}",
                std::str::from_utf8(&replacement).unwrap_or("????")
            );
            stsd[entry_pos + 4] = replacement[0];
            stsd[entry_pos + 5] = replacement[1];
            stsd[entry_pos + 6] = replacement[2];
            stsd[entry_pos + 7] = replacement[3];

            neutralize_sinf(&mut stsd[entry_pos..entry_pos + entry_sz]);
        }

        entry_pos += entry_sz;
    }
}

fn find_original_codec(entry: &[u8]) -> Option<[u8; 4]> {
    let mut pos = 36;
    while pos + 8 <= entry.len() {
        let sz = u32::from_be_bytes([entry[pos], entry[pos + 1], entry[pos + 2], entry[pos + 3]])
            as usize;
        let typ = &entry[pos + 4..pos + 8];
        if sz < 8 || pos + sz > entry.len() {
            break;
        }

        if typ == b"sinf" {
            return find_frma(&entry[pos..pos + sz]);
        }
        pos += sz;
    }
    None
}

fn find_frma(sinf: &[u8]) -> Option<[u8; 4]> {
    let mut pos = 8;
    while pos + 8 <= sinf.len() {
        let sz =
            u32::from_be_bytes([sinf[pos], sinf[pos + 1], sinf[pos + 2], sinf[pos + 3]]) as usize;
        let typ = &sinf[pos + 4..sinf.len().min(pos + 8)];
        if sz < 8 || pos + sz > sinf.len() {
            break;
        }

        if typ == b"frma" && sz >= 12 {
            return Some([sinf[pos + 8], sinf[pos + 9], sinf[pos + 10], sinf[pos + 11]]);
        }
        pos += sz;
    }
    None
}

fn neutralize_sinf(entry: &mut [u8]) {
    let mut pos = 36;
    while pos + 8 <= entry.len() {
        let sz = u32::from_be_bytes([entry[pos], entry[pos + 1], entry[pos + 2], entry[pos + 3]])
            as usize;
        if sz < 8 || pos + sz > entry.len() {
            break;
        }

        if &entry[pos + 4..pos + 8] == b"sinf" {
            entry[pos + 4] = b'f';
            entry[pos + 5] = b'r';
            entry[pos + 6] = b'e';
            entry[pos + 7] = b'e';
            debug!("neutralized sinf box at offset {pos}");
        }
        pos += sz;
    }
}
pub fn extract_flac_stream_header(moov: &[u8]) -> Option<Vec<u8>> {
    let dfla = find_dfla_in_moov(moov)?;

    if dfla.len() < 12 {
        return None;
    }

    let metadata_blocks = &dfla[12..];
    if metadata_blocks.is_empty() {
        return None;
    }

    if metadata_blocks[0] & 0x7F != 0 {
        warn!(
            "Amazon FLAC: unexpected first metadata block type {}",
            metadata_blocks[0] & 0x7F
        );
        return None;
    }

    let mut out = Vec::with_capacity(4 + metadata_blocks.len());
    out.extend_from_slice(b"fLaC");
    out.extend_from_slice(metadata_blocks);

    ensure_last_metadata_block_flag(&mut out[4..]);

    Some(out)
}

fn ensure_last_metadata_block_flag(metadata_blocks: &mut [u8]) {
    let mut block_starts: Vec<usize> = Vec::new();
    let mut pos = 0;

    while pos + 4 <= metadata_blocks.len() {
        let length = u32::from_be_bytes([
            0,
            metadata_blocks[pos + 1],
            metadata_blocks[pos + 2],
            metadata_blocks[pos + 3],
        ]) as usize;

        if pos + 4 + length > metadata_blocks.len() {
            break;
        }

        block_starts.push(pos);

        pos += 4 + length;
    }

    if block_starts.is_empty() {
        return;
    }

    for &start in &block_starts {
        metadata_blocks[start] &= 0x7F;
    }
    let last = *block_starts.last().unwrap();
    metadata_blocks[last] |= 0x80;
}

fn find_dfla_in_moov(moov: &[u8]) -> Option<&[u8]> {
    let trak = find_child(moov, b"trak")?;
    let mdia = find_child(trak, b"mdia")?;
    let minf = find_child(mdia, b"minf")?;
    let stbl = find_child(minf, b"stbl")?;
    let stsd = find_child(stbl, b"stsd")?;

    if stsd.len() < 16 {
        return None;
    }

    let audio_entry = {
        let entries = &stsd[16..];
        let mut pos = 0;
        loop {
            if pos + 8 > entries.len() {
                return None;
            }
            let sz = u32::from_be_bytes([
                entries[pos],
                entries[pos + 1],
                entries[pos + 2],
                entries[pos + 3],
            ]) as usize;
            if sz < 8 || pos + sz > entries.len() {
                return None;
            }
            let tag = &entries[pos + 4..pos + 8];
            if tag == b"enca" || tag == b"fLaC" {
                break &entries[pos..pos + sz];
            }
            pos += sz;
        }
    };

    if audio_entry.len() < 36 {
        return None;
    }

    find_box_in(&audio_entry[36..], b"dfLa")
}

fn find_child<'a>(parent: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
    if parent.len() < 8 {
        return None;
    }
    find_box_in(&parent[8..], target)
}

fn find_box_in<'a>(data: &'a [u8], target: &[u8; 4]) -> Option<&'a [u8]> {
    let mut pos = 0;
    while pos + 8 <= data.len() {
        let sz =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        if sz < 8 || pos + sz > data.len() {
            break;
        }
        if &data[pos + 4..pos + 8] == target {
            return Some(&data[pos..pos + sz]);
        }
        pos += sz;
    }
    None
}
