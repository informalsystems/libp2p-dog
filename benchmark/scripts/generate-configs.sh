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

NUM_MACHINES=$(grep -v '^$' ${MACHINES} | wc -l | tr -d ' ')
if [ ${NUM_NODES} -ne ${NUM_MACHINES} ]; then
    echo "Number of nodes (${NUM_NODES}) does not match number of machines (${NUM_MACHINES})"
    exit 1
fi

echo "Generating configs for ${NUM_NODES} nodes"

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

rm -rf ${OUTPUT}/*-*.toml

for i in $(seq 1 ${NUM_NODES}); do
  echo "Generating config for node ${i}"

  output="${OUTPUT}/${ID_PREFIX}-${i}.toml"
  id="${ID_PREFIX}-${i}"
  ip=$(sed -n "${i}p" ${MACHINES})
  addr="/ip4/${ip}/tcp/${PORT}"
  peers=""
  for j in $(seq 1 ${NUM_NODES}); do
    if [ ${i} -ne ${j} ]; then
      peer="/ip4/$(sed -n "${j}p" ${MACHINES})/tcp/${PORT}"
      peers="${peers} ${peer}"
    fi
  done

  generate_node_config ${output} ${id} ${addr} ${peers}
done

echo "Done"
