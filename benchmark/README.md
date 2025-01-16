# DOG benchmark setup

1. Spawn the machines you want and place their IPs in the `machines.txt` file (**PLEASE ADD A FINAL EMPTY LINE**).
2. Run the command `./scripts/generate-inventory.sh --machines machines.txt --output inventory.ini` to generate the inventory file.
3. Adjust the benchmark parameters (example in `sample-benchmark-config.conf`) in a file called `benchmark.conf`.
4. Run the command `./scripts/generate-configs.sh --config benchmark.conf --machines machines.txt --output config`
5. Initialize the machines with the command `ansible-playbook -i inventory.ini --forks $(cat machines.txt | wc -l) playbooks/init.yml`
6. Build the benchmark container with the command `ansible-playbook -i inventory.ini --forks $(cat machines.txt | wc -l) playbooks/build.yml`
7. Export the configuration on all machines with the command `ansible-playbook -i inventory.ini --forks $(cat machines.txt | wc -l) playbooks/config.yml`
8. Run the benchmark with the command `ansible-playbook -i inventory.ini --forks $(cat machines.txt | wc -l) playbooks/start.yml`
