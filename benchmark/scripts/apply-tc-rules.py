#!/usr/bin/python3
# INTENDED TO BE EXECUTED ON BENCHMARK MACHINES
import sys
import csv
import subprocess

def read_matrix(csv_file):
    with open(csv_file) as f:
        reader = csv.reader(f)
        header = next(reader)[1:]
        matrix = {}
        for row in reader:
            row_zone = row[0]
            matrix[row_zone] = {}
            for i, col_zone in enumerate(header):
                matrix[row_zone][col_zone] = int(row[i+1])
    return header, matrix

def read_ips(ip_file):
    with open(ip_file) as f:
        return [line.strip() for line in f if line.strip()]

def execute_command(cmd):
    subprocess.run(cmd, shell=True, check=True)

def build_tc_commands(header, matrix, ips, local_ip):
    commands = []
    num_zones = len(header)
    local_index = ips.index(local_ip)
    local_zone = header[local_index % num_zones]
    
    commands.append("tc qdisc del dev eth0 root 2> /dev/null || true")
    commands.append("tc qdisc add dev eth0 root handle 1: htb default 10")
    commands.append("tc class add dev eth0 parent 1: classid 1:1 htb rate 1gbit")
    commands.append("tc class add dev eth0 parent 1:1 classid 1:10 htb rate 1gbit")
    commands.append("tc qdisc add dev eth0 parent 1:10 handle 10: sfq perturb 10")
    
    handle = 11
    for zone in header:
        zone_machines = []
        for ip in ips:
            if ip != local_ip:
                idx = ips.index(ip)
                z = header[idx % num_zones]
                if z == zone:
                    zone_machines.append(ip)

        if not zone_machines:
            continue
        
        latency = matrix[local_zone][zone]
        if latency > 0:
            delta = latency // 20
            if delta == 0:
                delta = 1
            commands.append(f"tc class add dev eth0 parent 1:1 classid 1:{handle} htb rate 1gbit")
            commands.append(f"tc qdisc add dev eth0 parent 1:{handle} handle {handle}: netem delay {latency}ms {delta}ms distribution normal")
            for ip in zone_machines:
                commands.append(f"tc filter add dev eth0 protocol ip parent 1: prio 1 u32 match ip dst {ip}/32 flowid 1:{handle}")
            handle += 1
    return commands

def main():
    csv_file = sys.argv[1]
    ip_file = sys.argv[2]
    local_ip = sys.argv[3]
    
    header, matrix = read_matrix(csv_file)
    ips = read_ips(ip_file)
    commands = build_tc_commands(header, matrix, ips, local_ip)
    
    for cmd in commands:
        execute_command(cmd)

if __name__ == "__main__":
    main()
