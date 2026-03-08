use serde_json::Value;

const VK_B64_TABLE: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN0PQRSTUVWXYZO123456789+/=";

pub fn unmask_vk_url(url: &str, user_id: i64) -> String {
    if !url.contains("audio_api_unavailable") {
        return url.to_string();
    }

    let decode = || -> Option<String> {
        let extra = url.split("?extra=").nth(1)?;
        let (enc_url, enc_ops) = extra.split_once('#')?;

        let mut chars: Vec<char> = vk_b64_decode(enc_url).chars().collect();
        let ops_str = vk_b64_decode(enc_ops);
        let ops: Vec<&str> = ops_str.split('\x0b').collect();

        if ops.len() < 2 {
            return None;
        }

        let seed: i64 = ops[1].parse().ok()?;
        let mut index = seed ^ user_id;
        let len = chars.len();
        let mut swap_indices = vec![0usize; len];

        for n in (0..len).rev() {
            index = ((len as i64 * (n as i64 + 1)) ^ (index + n as i64)) % len as i64;
            swap_indices[n] = index.unsigned_abs() as usize;
        }

        for n in 1..len {
            chars.swap(n, swap_indices[len - 1 - n]);
        }

        Some(chars.iter().collect())
    };

    decode().unwrap_or_else(|| url.to_string())
}

fn vk_b64_decode(encoded: &str) -> String {
    let table: Vec<char> = VK_B64_TABLE.chars().collect();
    let mut out = String::new();
    let mut acc = 0u32;
    let mut bits = 0u32;

    for ch in encoded.chars() {
        let Some(pos) = table.iter().position(|&c| c == ch) else {
            continue;
        };
        acc = (acc << 6) | pos as u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push(char::from_u32((acc >> bits) & 0xFF).unwrap_or('\0'));
        }
    }

    out
}

pub fn extract_thumbnail(item: &Value) -> Option<String> {
    let thumb = &item["album"]["thumb"];
    ["photo_1200", "photo_600", "photo_300"]
        .iter()
        .find_map(|&key| {
            thumb[key]
                .as_str()
                .filter(|u| !u.is_empty())
                .map(String::from)
        })
}
