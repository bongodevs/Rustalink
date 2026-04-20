#!/bin/bash
set -e

cd /home/container

INTERNAL_IP=$(ip route get 1 | awk '{print $(NF-2);exit}')
export INTERNAL_IP

cargo --version

MODIFIED_STARTUP=$(echo -e "$(echo -e "${STARTUP}" | sed -e 's/{{/${/g' -e 's/}}/}/g')")
echo -e ":/home/container$ ${MODIFIED_STARTUP}"

eval "${MODIFIED_STARTUP}"
