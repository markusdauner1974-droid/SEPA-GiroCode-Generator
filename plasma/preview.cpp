#include "preview.h"

#include <QApplication>
#include <QClipboard>
#include <QGraphicsItemGroup>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QMouseEvent>
#include <QShortcut>
#include <QStyle>

#include <algorithm>

Preview::Preview(QWidget *parent) : QGraphicsView(new QGraphicsScene, parent)
{
    scene()->setParent(this);
    setFocusPolicy(Qt::StrongFocus);
    setAlignment(Qt::AlignHCenter | Qt::AlignTop);
    setRenderHint(QPainter::SmoothPixmapTransform);
    setBackgroundRole(QPalette::Window);
    setAccessibleName(tr("Rechnungsvorschau"));
    setAccessibleDescription(tr("Ziehen, um Text zu markieren. Mit Strg+C kopieren."));
    auto *copy = new QShortcut(QKeySequence::Copy, this);
    copy->setContext(Qt::WidgetWithChildrenShortcut);
    connect(copy, &QShortcut::activated, this, &Preview::copySelection);
}

void Preview::showPage(const Document &nextDocument, std::size_t page)
{
    document = nextDocument;
    pageIndex = page;
    scene()->clear();
    highlights = nullptr;
    pageSize = {};
    words.clear();
    selection = {};
    selecting = false;
    hasSelection = false;
    if (!document) {
        scene()->setSceneRect(QRectF());
        return;
    }
    const auto pixmap = QPixmap::fromImage(QImage::fromData(takeBytes(giro_page(document.get(), page))));
    if (pixmap.isNull())
        return;
    pageSize = pixmap.size();
    scene()->addPixmap(pixmap);
    scene()->setSceneRect(QRectF(QPointF(), QSizeF(pageSize)));
    const auto boxes = QJsonDocument::fromJson(takeBytes(giro_words(document.get(), page))).array();
    for (const auto &value : boxes) {
        const auto box = value.toArray();
        words.append(QRectF(QPointF(box[0].toDouble(), box[1].toDouble()),
                            QPointF(box[2].toDouble(), box[3].toDouble())));
    }
    viewport()->setCursor(words.empty() ? Qt::ArrowCursor : Qt::IBeamCursor);
    if (fitsWidth)
        fitWidth();
    centerOn(pageSize.width() / 2., 0.);
}

void Preview::fitWidth()
{
    fitsWidth = true;
    if (pageSize.isEmpty())
        return;
    const int scrollbar = style()->pixelMetric(QStyle::PM_ScrollBarExtent);
    const qreal factor = std::max(1, contentsRect().width() - scrollbar - 4) / qreal(pageSize.width());
    setTransform(QTransform::fromScale(factor, factor));
}

void Preview::zoomBy(qreal factor)
{
    if (pageSize.isEmpty())
        return;
    fitsWidth = false;
    const qreal next = std::clamp(transform().m11() * factor, 0.1, 6.);
    setTransformationAnchor(QGraphicsView::AnchorViewCenter);
    setTransform(QTransform::fromScale(next, next));
}

void Preview::resizeEvent(QResizeEvent *event)
{
    QGraphicsView::resizeEvent(event);
    if (fitsWidth)
        fitWidth();
}

QPointF Preview::pagePoint(const QPoint &point) const
{
    const QPointF location = mapToScene(point);
    return {std::clamp(location.x() / pageSize.width(), 0., 1.),
            std::clamp(location.y() / pageSize.height(), 0., 1.)};
}

void Preview::mousePressEvent(QMouseEvent *event)
{
    if (event->button() != Qt::LeftButton || words.empty()) {
        QGraphicsView::mousePressEvent(event);
        return;
    }
    setFocus(Qt::MouseFocusReason);
    selecting = true;
    anchor = pagePoint(event->pos());
    selectTo(event->pos());
    event->accept();
}

void Preview::mouseMoveEvent(QMouseEvent *event)
{
    if (selecting)
        selectTo(event->pos());
    else
        QGraphicsView::mouseMoveEvent(event);
}

void Preview::mouseReleaseEvent(QMouseEvent *event)
{
    if (selecting && event->button() == Qt::LeftButton) {
        selectTo(event->pos());
        selecting = false;
    } else {
        QGraphicsView::mouseReleaseEvent(event);
    }
}

void Preview::selectTo(const QPoint &point)
{
    selection = QRectF(anchor, pagePoint(point)).normalized();
    hasSelection = true;
    delete highlights;
    highlights = new QGraphicsItemGroup;
    scene()->addItem(highlights);
    QColor color = palette().highlight().color();
    color.setAlpha(100);
    for (const auto &word : words) {
        if (selection.left() > word.right() || selection.right() < word.left()
            || selection.top() > word.bottom() || selection.bottom() < word.top())
            continue;
        const QRectF painted(word.x() * pageSize.width(), word.y() * pageSize.height(),
                             word.width() * pageSize.width(), word.height() * pageSize.height());
        auto *box = new QGraphicsRectItem(painted, highlights);
        box->setPen(Qt::NoPen);
        box->setBrush(color);
    }
}

void Preview::copySelection() const
{
    if (!document || !hasSelection)
        return;
    const auto text = takeBytes(giro_copy(document.get(), pageIndex, selection.left(), selection.top(),
                                          selection.right(), selection.bottom()));
    if (!text.isEmpty())
        QApplication::clipboard()->setText(QString::fromUtf8(text));
}
