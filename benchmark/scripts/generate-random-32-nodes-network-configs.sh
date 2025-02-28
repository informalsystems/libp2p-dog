#!/bin/bash

while [[ $# -gt 0 ]]; do
    key="${1}"
    case $key in
        -c|--config)
            CONFIG="${2}"
            shift
            shift
            ;;
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

if [ -z "${CONFIG}" ] || [ -z "${MACHINES}" ] || [ -z "${OUTPUT}" ]; then
    echo "Usage: ${0} -c|--config <config> -m|--machines <machines> -o|--output <output>"
    exit 1
fi

source $CONFIG

if [ ! ${NUM_NODES} -ne 32 ]; then
    echo "NUM_NODES must be 32"
    exit 1
fi

NUM_MACHINES=$(grep -v '^$' ${MACHINES} | wc -l | tr -d ' ')
if [ ${NUM_NODES} -ne ${NUM_MACHINES} ]; then
    echo "Number of nodes (${NUM_NODES}) does not match number of machines (${NUM_MACHINES})"
    exit 1
fi

echo "Generating configs for ${NUM_NODES} nodes"

# These connections were obtained via the MPO protocol with Kademlia.
# Please refer to the chapter 3 of this thesis: https://bastienfaivre.com/files/master-thesis.pdf
declare -A PEER_MAP
PEER_MAP=(
  [1]="2 3 4 5 6 7 8 9 10 11"
  [2]="1 3 4 5 6 7 8 9 10 11"
  [3]="1 2 4 5 6 7 8 9 10 11"
  [4]="1 2 3 5 6 7 8 9 10 11"
  [5]="1 2 3 4 6 7 8 9 10 11"
  [6]="1 2 3 4 5 7 8 9 10 11"
  [7]="1 2 3 4 5 6 8 9 10 11"
  [8]="1 2 3 4 5 6 7 9 10 11"
  [9]="1 2 3 4 5 6 7 8 10 11"
  [10]="1 2 3 4 5 6 7 8 9 11"
  [11]="1 2 3 4 5 6 7 8 9 10"
  [12]="4 7 2 8 3 9 11 6 10 5"
  [13]="12 3 7 8 9 5 1 4 2 10"
  [14]="10 7 11 2 3 12 13 1 4 8"
  [15]="4 8 14 11 9 12 1 13 10 5"
  [16]="4 6 8 9 1 10 15 14 13 2"
  [17]="12 3 9 6 13 16 7 11 2 14"
  [18]="14 7 16 2 9 6 4 3 13 15"
  [19]="1 7 8 18 9 11 5 12 16 6"
  [20]="18 16 3 13 19 17 15 2 8 10"
  [21]="7 6 3 1 5 18 4 8 2 11"
  [22]="2 18 10 6 11 14 20 13 3 5"
  [23]="21 6 2 11 3 19 12 15 1 18"
  [24]="18 6 16 13 9 12 7 19 8 3"
  [25]="3 4 18 10 9 8 1 21 5 23"
  [26]="13 9 20 7 21 8 14 23 6 18"
  [27]="25 18 17 26 19 20 24 4 1 9"
  [28]="13 15 22 4 26 27 2 5 7 21"
  [29]="3 8 21 18 9 14 13 24 16 12"
  [30]="17 12 22 27 18 2 28 21 20 5"
  [31]="4 15 24 21 12 1 2 29 26 7"
  [32]="19 24 16 4 28 8 26 25 6 30"
)

rm -rf ${OUTPUT}/*-*.toml

for i in $(seq 1 ${NUM_NODES}); do
  echo "Generating config for node ${i}"

  output="${OUTPUT}/${ID_PREFIX}-${i}.toml"
  id="${ID_PREFIX}-${i}"
  ip=$(sed -n "${i}p" ${MACHINES})
  addr="/ip4/${ip}/tcp/${PORT}"
  peers=""
  
  for j in ${PEER_MAP[$i]}; do
    peer_ip=$(sed -n "${j}p" ${MACHINES})
    peer="/ip4/${peer_ip}/tcp/${PORT}"
    peers="${peers} ${peer}"
  done

  generate_node_config() {
    output=${1}
    id=${2}
    addr=${3}
    peers="${@:4}"

    touch ${output}
    > ${output}

    echo "[node]" >> ${output}
    echo "id = \"${id}\"" >> ${output}
    echo "addr = \"${addr}\"" >> ${output}
    echo "peers = [" >> ${output}
    for peer in ${peers}; do
      echo "  \"${peer}\"," >> ${output}
    done
    echo -e "]\n" >> ${output}

    echo "[benchmark]" >> ${output}
    echo "protocol = \"${PROTOCOL}\"" >> ${output}
    echo "duration_in_sec = ${DURATION}" >> ${output}
    echo "tps = ${TPS}" >> ${output}
    echo "tx_size_in_bytes = ${TX_SIZE}" >> ${output}
    echo "dump_interval_in_ms = ${DUMP_INTERVAL}" >> ${output}
    echo "registry_prefix = \"${REGISTRY_PREFIX}\"" >> ${output}
    echo "redundancy = ${REDUNDANCY}" >> ${output}
    echo "redundancy_delta = ${DELTA}" >> ${output}
    echo "redundancy_interval_in_ms = ${REDUNDANCY_INTERVAL}" >> ${output}
    echo "stop_delay_in_sec = ${STOP_DELAY}" >> ${output}
  }

  generate_node_config ${output} ${id} ${addr} ${peers}
done

echo "Done"