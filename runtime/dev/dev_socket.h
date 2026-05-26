#pragma once
#include <string>
#include <functional>

class DevSocket {
public:
    DevSocket();
    ~DevSocket();

    bool listen(const std::string& path = "/tmp/morph_dev.sock");
    bool acceptClient();
    bool readMessage(std::string& out, int timeoutMs = 0);
    bool sendMessage(const std::string& msg);
    bool isConnected() const { return m_client >= 0; }
    void close();

private:
    int m_sock = -1;
    int m_client = -1;
    std::string m_path;
    std::string m_recvBuf;
};
