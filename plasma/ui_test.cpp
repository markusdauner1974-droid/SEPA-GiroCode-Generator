#define GIRO_UI_TEST
#include "main.cpp"

#include <QClipboard>
#include <QPainter>
#include <QPdfWriter>
#include <QTemporaryDir>
#include <QtTest>

class PlasmaWorkflow : public QObject {
    Q_OBJECT
private slots:
    void openEditAndGenerate();
};

void PlasmaWorkflow::openEditAndGenerate()
{
    QTemporaryDir temp;
    QVERIFY(temp.isValid());
    QSettings::setDefaultFormat(QSettings::IniFormat);
    QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, temp.path());
    QApplication::setOrganizationName("GiroCodeTest");
    QApplication::setApplicationName("PlasmaWorkflow");

    const QString path = temp.filePath("beispiel.pdf");
    {
        QPdfWriter pdf(path);
        pdf.setResolution(72);
        QPainter painter(&pdf);
        painter.drawText(30, 42, "Empfänger: Beispiel GmbH");
        painter.drawText(30, 65, "IBAN: DE89 3704 0044 0532 0130 00");
        painter.drawText(30, 88, "Zahlbetrag: 24,90");
        painter.drawText(30, 111, "Rechnungsnummer: TEST-17");
        QVERIFY(painter.end());
    }

    Window window;
    window.show();
    QMimeData mime;
    mime.setUrls({QUrl::fromLocalFile(path)});
    QDragEnterEvent drag(QPoint(40, 100), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QApplication::sendEvent(&window, &drag);
    QVERIFY(drag.isAccepted());
    QDropEvent drop(QPointF(40, 100), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QApplication::sendEvent(&window, &drop);
    QVERIFY(drop.isAccepted());

    auto *amount = window.findChild<QLineEdit *>("paymentField3");
    auto *purpose = window.findChild<QLineEdit *>("paymentField4");
    auto *generate = window.findChild<QPushButton *>("generateCode");
    auto *qr = window.findChild<QLabel *>("giroCodeImage");
    QVERIFY(amount && purpose && generate && qr);
    QTRY_COMPARE(amount->text(), QString("24,90"));
    amount->setFocus();
    QTest::keyClick(amount, Qt::Key_Tab);
    QCOMPARE(QApplication::focusWidget(), purpose);
    QTest::mouseClick(generate, Qt::LeftButton);
    QVERIFY(qr->isVisible());
    QVERIFY(!qr->pixmap().isNull());
    QTest::keyClick(amount, Qt::Key_End);
    QTest::keyClick(amount, Qt::Key_Backspace);
    QTest::keyClicks(amount, "5");
    QCOMPARE(amount->text(), QString("24,95"));
    QVERIFY(qr->isHidden());

    auto *preview = window.findChild<QGraphicsView *>();
    QVERIFY(preview && !preview->sceneRect().isEmpty());
    const QRectF rect = preview->sceneRect();
    const auto from = preview->mapFromScene(rect.width() * 0.05, rect.height() * 0.04);
    const auto to = preview->mapFromScene(rect.width() * 0.62, rect.height() * 0.08);
    QTest::mousePress(preview->viewport(), Qt::LeftButton, Qt::NoModifier, from);
    QTest::mouseMove(preview->viewport(), to);
    QTest::mouseRelease(preview->viewport(), Qt::LeftButton, Qt::NoModifier, to);
    QTest::keyClick(preview, Qt::Key_C, Qt::ControlModifier);
    QVERIFY(QApplication::clipboard()->text().contains("Beispiel GmbH"));
}

QTEST_MAIN(PlasmaWorkflow)
#include "ui_test.moc"
