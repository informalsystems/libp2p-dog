#!/bin/bash

while [[ $# -gt 0 ]]; do
    key="${1}"
    case $key in
        -m|--machines)
            MACHINES="${2}"
            shift
            shift
            ;;
        -o|--output)
            OUTPUT="${2}"
            shift
            shift
            ;;
        *)
            echo "Unknown argument: ${1}"
            exit 1
            ;;
    esac
done

if [ -z "${MACHINES}" ] || [ -z "${OUTPUT}" ]; then
    echo "Usage: ${0} -m|--machines <machines> -o|--output <output>"
    exit 1
fi

echo "Filling in the ansible inventory file"

# check that the output file exists
if [ ! -f "${OUTPUT}" ]; then
    touch "${OUTPUT}"
    echo "[machines]" > "${OUTPUT}"
    echo "" >> "${OUTPUT}"
    echo "[machines:vars]" >> "${OUTPUT}"
    echo "ansible_ssh_user=bob" >> "${OUTPUT}"
fi

awk -v machines="${MACHINES}" '
NR==FNR {
    new_ips[NR] = $0;
    next;
}
{
    if ($0 ~ /^\[machines\]/) {
        print;
        for (i in new_ips) {
            print i " ansible_host=" new_ips[i];
        }
        print ""; # Add an empty line after the entries
        while (getline > 0 && $0 !~ /^\[machines:vars\]/) {}
    }
    if ($0 ~ /^\[machines:vars\]/ || FNR != 1) {
        print;
    }
}' "${MACHINES}" "${OUTPUT}" > "${OUTPUT}.tmp"
mv "${OUTPUT}.tmp" "${OUTPUT}"

echo "Done"
