#include "net.h"
#include "../dev/dev_net.h"
 
// ── Socket portability ────────────────────────────────────────────
// One implementation of the HTTP stack serves every OS; only the socket
// primitives differ (WinSock vs POSIX sockets).
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
using SocketT = SOCKET;
static constexpr SocketT kInvalidSocket = INVALID_SOCKET;
inline bool validSocket(SocketT s) { return s != kInvalidSocket; }
inline void closeSocket(SocketT s) { ::closesocket(s); }
namespace {
struct WinsockInit {
    WinsockInit() {
        WSADATA data;
        WSAStartup(MAKEWORD(2, 2), &data);
    }
} g_winsockInit;
} // anonymous namespace
#else
#include <netdb.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <unistd.h>
using SocketT = int;
static constexpr SocketT kInvalidSocket = -1;
inline bool validSocket(SocketT s) { return s >= 0; }
inline void closeSocket(SocketT s) { ::close(s); }
#endif

#include <cstring>
#include <sstream>
#include <thread>

namespace morph::net {

namespace detail {

void HttpAwaitable::await_suspend(std::coroutine_handle<> h) noexcept {
    std::shared_ptr<SharedState> st = state;
    std::thread([st, h]() mutable {
        #ifdef MORPH_FEATURE_DEV 
        int netId = devNetBegin(st->method.c_str(), st->url.c_str());
        #endif
        st->response = http_request(st->url, st->method, st->requestHeaders, st->requestBody);
        std::string err;
        if (st->response.status == 0) {
            err = "Failed to fetch " + st->url +
                  " (network error: no connection or empty reply)";
            st->error = err;
        }
        std::string respHead;
        respHead += "HTTP/1.1 " + std::to_string(st->response.status) + "\r\n";
        for (const auto& [k, v] : st->response.headers)
            respHead += k + ": " + v + "\r\n";
        static const size_t kMaxPreview = 8192;
        std::string preview = st->response.body;
        if (preview.size() > kMaxPreview)
            preview.resize(kMaxPreview);
        #ifdef MORPH_FEATURE_DEV 
        devNetEnd(netId, st->response.status, st->response.body.size(), err,
                  st->response.requestHead, respHead, preview);
        #endif
        h.resume();
        // If the coroutine completed during resume, reclaim its frame.
        // Covers the common fire-and-forget pattern where the caller
        // discards the morph::Result without awaiting it.
        if (h.done()) {
            h.destroy();
        }
    }).detach();
}

} // namespace detail

namespace {

struct UrlParts {
    std::string host;
    int port = 80;
    std::string path = "/";
};

bool parse_url(const std::string& raw, UrlParts& out) {
    std::string rest = raw;
    std::string scheme;
    auto scheme_pos = rest.find("://");
    if (scheme_pos != std::string::npos) {
        scheme = rest.substr(0, scheme_pos);
        rest = rest.substr(scheme_pos + 3);
    }
    if (scheme == "https") {
        out.port = 443;
    }
    auto slash = rest.find('/');
    std::string authority = (slash == std::string::npos) ? rest : rest.substr(0, slash);
    if (slash != std::string::npos) {
        out.path = rest.substr(slash);
        if (out.path.empty()) {
            out.path = "/";
        }
    }
    auto colon = authority.find(':');
    if (colon != std::string::npos) {
        out.host = authority.substr(0, colon);
        try {
            out.port = std::stoi(authority.substr(colon + 1));
        } catch (...) {
            return false;
        }
    } else {
        out.host = authority;
    }
    return !out.host.empty();
}

std::string build_request(const UrlParts& parts, const std::string& host_header, const std::string& method, const Headers& headers, const std::string& body) {
    std::ostringstream req;
    req << method << " " << parts.path << " HTTP/1.1\r\n";
    req << "Host: " << host_header << "\r\n";
    req << "User-Agent: morph-net/0.1\r\n";
    req << "Accept: */*\r\n";
    // Custom headers
    for (auto& kv : headers.map) {
        req << kv.first << ": " << kv.second << "\r\n";
    }
    if (!body.empty()) {
        req << "Content-Length: " << body.size() << "\r\n";
    }
    req << "Connection: close\r\n\r\n";
    if (!body.empty()) req << body;
    return req.str();
}
std::string build_request(const UrlParts& parts, const std::string& host_header) {
    return build_request(parts, host_header, "GET", Headers{}, "");
}

std::string recv_all(SocketT fd) {
    std::string data;
    char buf[16384];
    int n;
    while ((n = ::recv(fd, buf, static_cast<int>(sizeof(buf)), 0)) > 0) {
        data.append(buf, static_cast<size_t>(n));
    }
    return data;
}

} // anonymous namespace

Response http_request(const std::string& url, const std::string& method, const Headers& headers, const std::string& body) {
    Response resp;
    resp.url = url;
    UrlParts parts;
    if (!parse_url(url, parts)) {
        resp.statusText = "Invalid URL";
        resp.type = "error";
        return resp;
    }

    addrinfo hints{};
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;

    addrinfo* res = nullptr;
    std::string port_str = std::to_string(parts.port);
    int gai = ::getaddrinfo(parts.host.c_str(), port_str.c_str(), &hints, &res);
    if (gai != 0) {
        return resp;
    }

    SocketT fd = kInvalidSocket;
    for (addrinfo* it = res; it != nullptr; it = it->ai_next) {
        fd = ::socket(it->ai_family, it->ai_socktype, it->ai_protocol);
        if (!validSocket(fd)) {
            continue;
        }
        if (::connect(fd, it->ai_addr, static_cast<int>(it->ai_addrlen)) == 0) {
            break;
        }
        closeSocket(fd);
        fd = kInvalidSocket;
    }
    ::freeaddrinfo(res);
    if (!validSocket(fd)) {
        return resp;
    }

    std::string host_header = parts.host;
    if (parts.port != 80 && parts.port != 443) {
        host_header += ":" + std::to_string(parts.port);
    }
    std::string req = build_request(parts, host_header, method, headers, body);
    resp.requestHead = req;
    if (::send(fd, req.data(), static_cast<int>(req.size()), 0) < 0) {
        closeSocket(fd);
        resp.statusText = "Network Error";
        resp.type = "error";
        return resp;
    }

    std::string raw = recv_all(fd);
    closeSocket(fd);
    if (raw.empty()) {
        resp.statusText = "Empty Reply";
        resp.type = "error";
        return resp;
    }

    // Split headers from body.
    auto sep = raw.find("\r\n\r\n");
    std::string head = (sep == std::string::npos) ? raw : raw.substr(0, sep);
    std::string resp_body = (sep == std::string::npos) ? "" : raw.substr(sep + 4);

    // Status line: HTTP/1.1 200 OK
    std::istringstream hs(head);
    std::string line;
    if (std::getline(hs, line)) {
        // Remove \r
        if (!line.empty() && line.back() == '\r') line.pop_back();
        std::istringstream ls(line);
        std::string http_ver;
        int status = 0;
        std::string statusText;
        if (ls >> http_ver >> status) {
            resp.status = status;
            // Rest of line is statusText
            std::getline(ls, statusText);
            // Trim leading space
            while (!statusText.empty() && statusText.front() == ' ') statusText.erase(statusText.begin());
            if (!statusText.empty() && statusText.back() == '\r') statusText.pop_back();
            resp.statusText = statusText;
            if (resp.statusText.empty()) {
                if (resp.status == 200) resp.statusText = "OK";
                else if (resp.status == 404) resp.statusText = "Not Found";
                else if (resp.status >= 300 && resp.status < 400) resp.statusText = "Redirect";
                else resp.statusText = std::to_string(resp.status);
            }
        }
        // Check for redirect
        if (resp.status >= 300 && resp.status < 400) {
            resp.redirected = true;
            resp.type = "cors";
        } else if (resp.status >= 200 && resp.status < 300) {
            resp.type = "basic";
        } else {
            resp.type = "basic";
        }
    }
    while (std::getline(hs, line)) {
        if (line.empty() || line == "\r") {
            continue;
        }
        auto colon = line.find(':');
        if (colon == std::string::npos) {
            continue;
        }
        std::string key = line.substr(0, colon);
        std::string val = line.substr(colon + 1);
        while (!val.empty() && (val.front() == ' ' || val.front() == '\t')) {
            val.erase(val.begin());
        }
        while (!val.empty() && (val.back() == '\r' || val.back() == '\n')) {
            val.pop_back();
        }
        resp.headers.set(key, val);
    }

    resp.body = std::move(resp_body);
    // bodyUsed stays false until text()/json() called
    return resp;
}

Response http_get(const std::string& url) {
    return http_request(url, "GET", Headers{}, "");
}

} // namespace morph::net
