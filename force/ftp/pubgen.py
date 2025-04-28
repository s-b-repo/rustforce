def is_public_ip(ip):
    octets = list(map(int, ip.split('.')))
    if (
        octets[0] == 10 or
        (octets[0] == 172 and 16 <= octets[1] <= 31) or
        (octets[0] == 192 and octets[1] == 168) or
        octets[0] == 127 or
        (octets[0] == 169 and octets[1] == 254) or
        (octets[0] >= 224)
    ):
        return False
    return True

def generate_all_public_ips():
    for a in range(1, 224):  # Avoid reserved ranges starting from 224
        for b in range(256):
            for c in range(256):
                for d in range(1, 255):  # Skip .0 and .255
                    ip = f"{a}.{b}.{c}.{d}"
                    if is_public_ip(ip):
                        yield ip

def save_all_public_ips(filename='ips.txt', chunk_size=100000):
    with open(filename, 'a') as file:
        buffer = []
        count = 0
        for ip in generate_all_public_ips():
            buffer.append(ip)
            count += 1
            if count % chunk_size == 0:
                file.write('\n'.join(buffer) + '\n')
                buffer = []  # Reset buffer
                print(f"{count} IPs appended to {filename}...")  # Progress update

        # Write any remaining IPs in the buffer
        if buffer:
            file.write('\n'.join(buffer) + '\n')

    print(f"Completed appending all public IPs to {filename}")

# Uncomment to run the process
save_all_public_ips()
