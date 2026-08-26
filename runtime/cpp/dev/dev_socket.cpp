#include "dev_socket.h"

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
#else
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>
#include <cerrno>
#include <fcntl.h>
#endif
#include <cstring>
#include <cstdio>

// Fixed loopback port shared with morph/dev/server.py (IPCClient).
static const int kDevPort = 39573;

static int lastError() {
#ifdef _WIN32
    return WSAGetLastError();
#else
    return errno;
#endif
}

static bool wouldBlock(int err) {
#ifdef _WIN32
    return err == WSAEWOULDBLOCK || err == WSAEINPROGRESS;
#else
    return err == EAGAIN || err == EWOULDBLOCK;
#endif
}

static void setNonBlocking(int fd) {
#ifdef _WIN32
    u_long mode = 1;
    ioctlsocket(fd, FIONBIO, &mode);
#else
    int flags = fcntl(fd, F_GETFL, 0);
    fcntl(fd, F_SETFL, flags | O_NONBLOCK);
#endif
}

static void closeFd(int fd) {
#ifdef _WIN32
    closesocket(fd);
#else
    ::close(fd);
#endif
}

DevSocket::DevSocket() {}

DevSocket::~DevSocket() { close(); }

bool DevSocket::listen() {
#ifdef _WIN32
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "[morph] dev socket: WSAStartup failed\n");
        return false;
    }
#endif
    m_sock = (int)socket(AF_INET, SOCK_STREAM, 0);
    if (m_sock < 0) {
        fprintf(stderr, "[morph] dev socket: socket failed\n");
        return false;
    }

    int opt = 1;
    setsockopt(m_sock, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons((u_short)kDevPort);
    addr.sin_addr.s_addr = inet_addr("127.0.0.1");

    if (bind(m_sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        fprintf(stderr, "[morph] dev socket: bind failed\n");
        close();
        return false;
    }
    if (::listen(m_sock, 1) < 0) {
        fprintf(stderr, "[morph] dev socket: listen failed\n");
        close();
        return false;
    }

    setNonBlocking(m_sock);
    return true;
}

bool DevSocket::acceptClient() {
    if (m_client >= 0) return true;
    m_client = (int)accept(m_sock, nullptr, nullptr);
    if (m_client < 0) {
        if (!wouldBlock(lastError()))
            fprintf(stderr, "[morph] dev socket: accept failed\n");
        return false;
    }
    setNonBlocking(m_client);
    return true;
}

bool DevSocket::readMessage(std::string& out, int timeoutMs) {
    if (m_client < 0) {
        out.clear();
        return false;
    }

    // select() is available on both POSIX and Winsock, so polling stays
    // portable while the socket itself remains non-blocking.
    fd_set rfds;
    FD_ZERO(&rfds);
#ifdef _WIN32
    FD_SET((SOCKET)m_client, &rfds);
#else
    FD_SET(m_client, &rfds);
#endif
    timeval tv;
    tv.tv_sec = timeoutMs / 1000;
    tv.tv_usec = (timeoutMs % 1000) * 1000;

    int ret = select(m_client + 1, &rfds, nullptr, nullptr, &tv);
    if (ret < 0) {
        return false;
    }
    if (ret == 0) {
        out.clear();
        return false; // timeout, no data
    }

    char buf[65536];
    int n = (int)recv(m_client, buf, sizeof(buf), 0);
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
        closeFd(m_client);
        m_client = -1;
    } else if (!wouldBlock(lastError())) {
        closeFd(m_client);
        m_client = -1;
    }

    out.clear();
    return false;
}

bool DevSocket::sendMessage(const std::string& msg) {
    if (m_client < 0) return false;
    std::string payload = msg + "\0";
    int n = (int)send(m_client, payload.data(), (int)payload.size(), 0);
    return n > 0;
}

void DevSocket::close() {
    if (m_client >= 0) {
        closeFd(m_client);
        m_client = -1;
    }
    if (m_sock >= 0) {
        closeFd(m_sock);
        m_sock = -1;
    }
    m_recvBuf.clear();
}
