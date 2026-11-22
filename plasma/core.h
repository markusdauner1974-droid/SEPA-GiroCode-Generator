#pragma once

#include <cstddef>
#include <memory>

#include <QByteArray>

struct Invoice;
struct RustBytes {
    unsigned char *data;
    std::size_t len;
};

extern "C" {
Invoice *giro_open(const char *path, std::size_t len, bool forceOcr);
void giro_close(Invoice *invoice);
RustBytes giro_details(const Invoice *invoice);
RustBytes giro_page(const Invoice *invoice, std::size_t index);
RustBytes giro_words(const Invoice *invoice, std::size_t index);
RustBytes giro_copy(const Invoice *invoice, std::size_t index, float left, float top,
                    float right, float bottom);
RustBytes giro_code(const char *input, std::size_t len);
void giro_free(RustBytes bytes);
}

using Document = std::shared_ptr<Invoice>;

inline QByteArray takeBytes(RustBytes bytes)
{
    QByteArray result(reinterpret_cast<const char *>(bytes.data), bytes.len);
    giro_free(bytes);
    return result;
}
