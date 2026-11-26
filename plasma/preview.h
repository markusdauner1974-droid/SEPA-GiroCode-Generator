#pragma once

#include "core.h"

#include <QGraphicsView>

class Preview : public QGraphicsView {
public:
    explicit Preview(QWidget *parent = nullptr);
    void showPage(const Document &document, std::size_t page);
    void zoomBy(qreal factor);
    void fitWidth();
    void copySelection() const;

protected:
    void resizeEvent(QResizeEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;

private:
    QPointF pagePoint(const QPoint &point) const;
    void selectTo(const QPoint &point);

    Document document;
    std::size_t pageIndex = 0;
    QSize pageSize;
    QVector<QRectF> words;
    QRectF selection;
    QPointF anchor;
    QGraphicsItemGroup *highlights = nullptr;
    bool selecting = false;
    bool hasSelection = false;
    bool fitsWidth = true;
};
