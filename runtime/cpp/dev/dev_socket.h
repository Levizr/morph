#pragma once
#include <string>

// Cross-platform loopback-TCP IPC between morph's Python dev driver and the
// morph_devrt runtime. 127.0.0.1 keeps it working on Linux, macOS and Windows
// without OS-specific socket APIs (no Unix sockets, no named pipes).
class DevSocket {
public:
    DevSocket();
    ~DevSocket();

    bool listen();
    bool acceptClient();
    bool readMessage(std::string& out, int timeoutMs = 0);
    bool sendMessage(const std::string& msg);
    bool isConnected() const { return m_client >= 0; }
    // Bound loopback port. The preferred port is tried first; when it is
    // taken the socket falls back to an OS-assigned ephemeral port, so
    // concurrent `morph dev` sessions never collide. Valid after listen().
    int port() const { return m_port; }
    void close();

private:
    int m_sock = -1;
    int m_client = -1;
    int m_port = -1;
    std::string m_recvBuf;
};
