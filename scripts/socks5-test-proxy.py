#!/usr/bin/env python3
"""A minimal SOCKS5 proxy, for testing that Flume actually routes through one.

Implements only what a client needs: the no-auth handshake and the CONNECT
command (RFC 1928). No UDP ASSOCIATE, no BIND, no authentication. That is
enough for librqbit, which uses SOCKS5 for outgoing TCP peer connections.

Every connection is logged, which is the entire point: seeing peer addresses
appear here is proof that traffic is going through the proxy rather than
direct.
"""
import socket, struct, threading, sys, time

HOST, PORT = "127.0.0.1", 1080
count = 0
lock = threading.Lock()

def pipe(a, b):
    try:
        while True:
            data = a.recv(65536)
            if not data:
                break
            b.sendall(data)
    except OSError:
        pass
    finally:
        for s in (a, b):
            try: s.close()
            except OSError: pass

def handle(client):
    global count
    try:
        # Greeting: version, number of methods, methods
        head = client.recv(2)
        if len(head) < 2 or head[0] != 0x05:
            client.close(); return
        client.recv(head[1])                      # discard offered methods
        client.sendall(b"\x05\x00")               # choose "no authentication"

        # Request: version, command, reserved, address type
        req = client.recv(4)
        if len(req) < 4 or req[1] != 0x01:        # 0x01 = CONNECT
            client.sendall(b"\x05\x07\x00\x01" + b"\x00" * 6)
            client.close(); return

        atyp = req[3]
        if atyp == 0x01:                          # IPv4
            host = socket.inet_ntoa(client.recv(4))
        elif atyp == 0x03:                        # domain name
            host = client.recv(client.recv(1)[0]).decode()
        elif atyp == 0x04:                        # IPv6
            host = socket.inet_ntop(socket.AF_INET6, client.recv(16))
        else:
            client.close(); return
        port = struct.unpack("!H", client.recv(2))[0]

        with lock:
            count += 1
            n = count
        target = f"[{host}]:{port}" if ":" in host else f"{host}:{port}"
        print(f"  #{n:<4} CONNECT {target}", flush=True)

        try:
            remote = socket.create_connection((host, port), timeout=10)
        except OSError:
            client.sendall(b"\x05\x05\x00\x01" + b"\x00" * 6)   # refused
            client.close(); return

        client.sendall(b"\x05\x00\x00\x01" + b"\x00" * 6)       # success
        threading.Thread(target=pipe, args=(client, remote), daemon=True).start()
        pipe(remote, client)
    except OSError:
        try: client.close()
        except OSError: pass

server = socket.socket()
server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
server.bind((HOST, PORT))
server.listen(128)
print(f"SOCKS5 test proxy listening on {HOST}:{PORT}", flush=True)
print("Each line below is a connection Flume made *through the proxy*.", flush=True)

try:
    while True:
        conn, _ = server.accept()
        threading.Thread(target=handle, args=(conn,), daemon=True).start()
except KeyboardInterrupt:
    print(f"\nTotal proxied connections: {count}", flush=True)
