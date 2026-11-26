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
    void dropInvoiceOnPreviewAndField();
};

void PlasmaWorkflow::dropInvoiceOnPreviewAndField()
{
    QTemporaryDir temp;
    QVERIFY(temp.isValid());
    auto makePdf = [&](const QString &name, const QString &amount) {
        const QString path = temp.filePath(name);
        QPdfWriter pdf(path);
        pdf.setResolution(72);
        QPainter painter(&pdf);
        painter.drawText(30, 42, "Empfänger: Beispiel GmbH");
        painter.drawText(30, 65, "IBAN: DE89 3704 0044 0532 0130 00");
        painter.drawText(30, 88, "Zahlbetrag: " + amount);
        painter.end();
        return path;
    };
    const auto first = makePdf("eins.pdf", "12,34");
    const auto second = makePdf("zwei.pdf", "56,78");
    Window window;
    window.show();
    auto *preview = window.findChild<QGraphicsView *>();
    auto *amount = window.findChild<QLineEdit *>("paymentField3");
    QVERIFY(preview && amount);
    auto dropFile = [](QWidget *target, const QString &path) {
        QMimeData mime;
        mime.setUrls({QUrl::fromLocalFile(path)});
        QDragEnterEvent enter(QPoint(5, 5), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
        QApplication::sendEvent(target, &enter);
        if (!enter.isAccepted()) return false;
        QDropEvent drop(QPointF(5, 5), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
        QApplication::sendEvent(target, &drop);
        return drop.isAccepted();
    };
    QVERIFY(dropFile(preview->viewport(), first));
    QTRY_COMPARE(amount->text(), QString("12,34"));
    QVERIFY(dropFile(amount, second));
    QTRY_COMPARE(amount->text(), QString("56,78"));
}

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
    auto *help = window.findChild<QMenu *>("helpMenu");
    auto *instructions = window.findChild<QAction *>("helpInstructions");
    auto *report = window.findChild<QAction *>("reportProblem");
    auto *about = window.findChild<QAction *>("aboutProgram");
    auto *licenses = window.findChild<QAction *>("openSourceLicenses");
    QVERIFY(help && instructions && report && about && licenses);
    QVERIFY(help->actions().contains(instructions));
    QVERIFY(help->actions().contains(report));
    QVERIFY(help->actions().contains(about));
    QVERIFY(help->actions().contains(licenses));
    QCOMPARE(instructions->shortcut(), QKeySequence(QKeySequence::HelpContents));
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
    auto *iban = window.findChild<QLineEdit *>("paymentField1");
    auto *generate = window.findChild<QPushButton *>("generateCode");
    auto *qr = window.findChild<QLabel *>("giroCodeImage");
    QVERIFY(amount && iban && purpose && generate && qr);
    QTRY_COMPARE(amount->text(), QString("24,90"));
    QCOMPARE(iban->text(), QString("DE89 3704 0044 0532 0130 00"));
    QCOMPARE(displayIban("FR1420041010050500013M02606"), QString("FR14 2004 1010 0505 0001 3M02 606"));
    const auto codeFor = [](const QString &value) {
        const QJsonArray fields{QStringLiteral("Beispiel GmbH"), value, QString(),
                                QStringLiteral("24,90"), QStringLiteral("Rechnung TEST-17")};
        const QByteArray data = QJsonDocument(fields).toJson(QJsonDocument::Compact);
        return takeBytes(giro_code(data.data(), data.size()));
    };
    const QByteArray grouped = codeFor(iban->text());
    QVERIFY(!QImage::fromData(grouped).isNull());
    QCOMPARE(grouped, codeFor("DE89370400440532013000"));
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
