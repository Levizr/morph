#include "dev_socket.h"
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cstring>
#include <cerrno>
#include <fcntl.h>
#include <poll.h>
#include <cstdio>

DevSocket::DevSocket() {}

DevSocket::~DevSocket() { close(); }

bool DevSocket::listen(const std::string& path) {
    m_path = path;
    m_sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (m_sock < 0) {
        perror("[morph] dev socket");
        return false;
    }

    int opt = 1;
    setsockopt(m_sock, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, path.c_str(), sizeof(addr.sun_path) - 1);

    unlink(path.c_str());
    if (bind(m_sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("[morph] dev socket bind");
        close();
        return false;
    }

    if (::listen(m_sock, 1) < 0) {
        perror("[morph] dev socket listen");
        close();
        return false;
    }

    // Non-blocking accept
    int flags = fcntl(m_sock, F_GETFL, 0);
    fcntl(m_sock, F_SETFL, flags | O_NONBLOCK);

    return true;
}

bool DevSocket::acceptClient() {
    if (m_client >= 0) return true;
    struct sockaddr_un addr;
    socklen_t len = sizeof(addr);
    m_client = accept(m_sock, (struct sockaddr*)&addr, &len);
    if (m_client < 0) {
        if (errno != EAGAIN && errno != EWOULDBLOCK)
            perror("[morph] dev socket accept");
        return false;
    }

    // Non-blocking reads
    int flags = fcntl(m_client, F_GETFL, 0);
    fcntl(m_client, F_SETFL, flags | O_NONBLOCK);

    return true;
}

bool DevSocket::readMessage(std::string& out, int timeoutMs) {
    if (m_client < 0) {
        out.clear();
        return false;
    }

    // Poll for data
    struct pollfd pfd;
    pfd.fd = m_client;
    pfd.events = POLLIN;
    int ret = poll(&pfd, 1, timeoutMs);
    if (ret < 0) {
        perror("[morph] dev socket poll");
        return false;
    }
    if (ret == 0) {
        out.clear();
        return false; // timeout, no data
    }

    // Check for errors
    if (pfd.revents & (POLLERR | POLLHUP | POLLNVAL)) {
        ::close(m_client);
        m_client = -1;
        out.clear();
        return false;
    }

    // Read available data
    char buf[65536];
    int n = (int)::read(m_client, buf, sizeof(buf));
    if (n > 0) {
        m_recvBuf.append(buf, n);

        // Extract null-terminated messages
        size_t pos;
        while ((pos = m_recvBuf.find('\0')) != std::string::npos) {
            out = m_recvBuf.substr(0, pos);
            m_recvBuf.erase(0, pos + 1);
            return true;
        }
    } else if (n == 0) {
        ::close(m_client);
        m_client = -1;
    } else if (errno != EAGAIN && errno != EWOULDBLOCK) {
        perror("[morph] dev socket read");
        ::close(m_client);
        m_client = -1;
    }

    out.clear();
    return false;
}

bool DevSocket::sendMessage(const std::string& msg) {
    if (m_client < 0) return false;
    std::string payload = msg + "\0";
    int n = (int)::write(m_client, payload.data(), payload.size());
    return n > 0;
}

void DevSocket::close() {
    if (m_client >= 0) {
        ::close(m_client);
        m_client = -1;
    }
    if (m_sock >= 0) {
        ::close(m_sock);
        m_sock = -1;
    }
    if (!m_path.empty()) {
        unlink(m_path.c_str());
        m_path.clear();
    }
    m_recvBuf.clear();
}
