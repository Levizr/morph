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
    void close();

private:
    int m_sock = -1;
    int m_client = -1;
    std::string m_recvBuf;
};
