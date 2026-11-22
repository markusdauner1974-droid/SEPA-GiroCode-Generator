#include "preview.h"

#include <QAction>
#include <QApplication>
#include <QComboBox>
#include <QDragEnterEvent>
#include <QFileDialog>
#include <QFile>
#include <QFileInfo>
#include <QFutureWatcher>
#include <QGroupBox>
#include <QIcon>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QLineEdit>
#include <QMainWindow>
#include <QMenuBar>
#include <QMimeData>
#include <QPushButton>
#include <QScrollArea>
#include <QSettings>
#include <QSplitter>
#include <QToolBar>
#include <QVBoxLayout>
#include <QtConcurrentRun>

#include <array>

namespace {
const QString appName = QStringLiteral("SEPA-GiroCode-Generator");
const QString appId = QStringLiteral("io.github.markusdauner1974_droid.SepaGiroCodeGenerator");

QLabel *plainLabel(const QString &text = {}, QWidget *parent = nullptr)
{
    auto *label = new QLabel(text, parent);
    label->setTextFormat(Qt::PlainText);
    label->setWordWrap(true);
    return label;
}

bool supported(const QUrl &url)
{
    if (!url.isLocalFile())
        return false;
    const QString extension = QFileInfo(url.toLocalFile()).suffix().toLower();
    return extension == "pdf" || extension == "png" || extension == "jpeg" || extension == "jpg";
}
}

class Window : public QMainWindow {
public:
    Window()
    {
        setWindowTitle(appName);
        setAcceptDrops(true);
        resize(1100, 740);

        auto *file = menuBar()->addMenu(tr("&Datei"));
        auto *view = menuBar()->addMenu(tr("&Ansicht"));
        auto *toolbar = addToolBar(tr("Rechnung"));
        toolbar->setObjectName("invoiceTools");
        toolbar->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        openAction = file->addAction(QIcon::fromTheme("document-open"), tr("&Öffnen …"), QKeySequence::Open,
                                     this, &Window::openDialog);
        toolbar->addAction(openAction);
        file->addAction(tr("&Schließen"), QKeySequence::Close, this, &QWidget::close);
        toolbar->addSeparator();
        previous = view->addAction(QIcon::fromTheme("go-previous"), tr("Vorherige Seite"), QKeySequence(Qt::Key_PageUp),
                              this, [this] { showPage(index - 1); });
        next = view->addAction(QIcon::fromTheme("go-next"), tr("Nächste Seite"), QKeySequence(Qt::Key_PageDown),
                              this, [this] { showPage(index + 1); });
        toolbar->addAction(previous);
        toolbar->addAction(next);
        pageLabel = plainLabel(tr("Keine Rechnung geöffnet"));
        toolbar->addWidget(pageLabel);
        toolbar->addSeparator();
        zoomIn = view->addAction(QIcon::fromTheme("zoom-in"), tr("Vergrößern"), QKeySequence::ZoomIn,
                              this, [this] { preview->zoomBy(1.25); });
        zoomOut = view->addAction(QIcon::fromTheme("zoom-out"), tr("Verkleinern"), QKeySequence::ZoomOut,
                              this, [this] { preview->zoomBy(0.8); });
        fit = view->addAction(QIcon::fromTheme("zoom-fit-width"), tr("Seitenbreite"), QKeySequence(Qt::CTRL | Qt::Key_0),
                              this, [this] { preview->fitWidth(); });
        for (auto *action : {zoomIn, zoomOut, fit})
            toolbar->addAction(action);

        auto *splitter = new QSplitter;
        splitter->setChildrenCollapsible(false);
        preview = new Preview;
        splitter->addWidget(preview);
        auto *scroll = new QScrollArea;
        scroll->setWidgetResizable(true);
        scroll->setMinimumWidth(285);
        auto *panel = new QWidget;
        auto *layout = new QVBoxLayout(panel);

        auto *box = new QGroupBox(tr("Zahlungsdaten"));
        auto *form = new QVBoxLayout(box);
        notice = plainLabel(tr("Öffne eine Rechnung oder ziehe sie aus Dolphin ins Fenster."));
        form->addWidget(notice);
        ibanChoices = new QComboBox;
        ibanChoices->setAccessibleName(tr("Gefundene IBANs"));
        ibanChoices->hide();
        form->addWidget(ibanChoices);
        connect(ibanChoices, &QComboBox::textActivated, this, [this](const QString &text) {
            fields[1]->setText(text);
            fields[1]->setFocus();
        });

        const std::array<QString, 5> labels = {
            tr("&Empfänger"), tr("&IBAN"), tr("BI&C"), tr("&Betrag in Euro"), tr("&Verwendungszweck")
        };
        for (std::size_t i = 0; i < fields.size(); ++i) {
            auto *label = new QLabel(labels[i]);
            fields[i] = new QLineEdit;
            fields[i]->setObjectName(QStringLiteral("paymentField%1").arg(i));
            label->setBuddy(fields[i]);
            form->addWidget(label);
            form->addWidget(fields[i]);
            hints[i] = plainLabel();
            hints[i]->hide();
            form->addWidget(hints[i]);
            connect(fields[i], &QLineEdit::textChanged, this, [this, i] {
                clearQr();
                if (!fields[i]->text().trimmed().isEmpty())
                    hints[i]->hide();
            });
        }

        generate = new QPushButton(QIcon::fromTheme("view-barcode-qr"), tr("&GiroCode erstellen"));
        generate->setObjectName("generateCode");
        connect(generate, &QPushButton::clicked, this, &Window::generateCode);
        form->addWidget(generate);
        layout->addWidget(box);
        qrLabel = plainLabel();
        qrLabel->setObjectName("giroCodeImage");
        qrLabel->setAccessibleName(tr("GiroCode zum Scannen"));
        qrLabel->setAlignment(Qt::AlignHCenter);
        qrLabel->hide();
        layout->addWidget(qrLabel);
        layout->addStretch();
        scroll->setWidget(panel);
        splitter->addWidget(scroll);
        splitter->setStretchFactor(0, 3);
        splitter->setStretchFactor(1, 1);
        settingsSplitter = splitter;
        setCentralWidget(splitter);
        splitter->setSizes({760, 340});
        if (settings.contains("geometry"))
            restoreGeometry(settings.value("geometry").toByteArray());
        if (settings.contains("splitter"))
            splitter->restoreState(settings.value("splitter").toByteArray());

        connect(&watcher, &QFutureWatcher<Document>::finished, this, &Window::loaded);
        refreshControls();
    }

    void openFile(const QString &path)
    {
        if (watcher.isRunning() || path.isEmpty())
            return;
        settings.setValue("directory", QFileInfo(path).absolutePath());
        filename = QFileInfo(path).fileName();
        notice->setText(tr("Rechnung wird gelesen …"));
        document.reset();
        count = 0;
        preview->showPage({}, 0);
        ibanChoices->hide();
        for (std::size_t i = 0; i < fields.size(); ++i) {
            fields[i]->clear();
            hints[i]->hide();
        }
        clearQr();
        const QByteArray encoded = QFile::encodeName(path);
        watcher.setFuture(QtConcurrent::run([encoded] {
            return Document(giro_open(encoded.data(), encoded.size()), giro_close);
        }));
        refreshControls();
    }

protected:
    void dragEnterEvent(QDragEnterEvent *event) override
    {
        const auto urls = event->mimeData()->urls();
        if (urls.size() == 1 && supported(urls.front()) && !watcher.isRunning())
            event->acceptProposedAction();
    }

    void dropEvent(QDropEvent *event) override
    {
        const auto urls = event->mimeData()->urls();
        if (urls.size() == 1 && supported(urls.front()) && !watcher.isRunning()) {
            event->acceptProposedAction();
            openFile(urls.front().toLocalFile());
        }
    }

    void closeEvent(QCloseEvent *event) override
    {
        settings.setValue("geometry", saveGeometry());
        settings.setValue("splitter", settingsSplitter->saveState());
        QMainWindow::closeEvent(event);
    }

private:
    void openDialog()
    {
        const QString chosen = QFileDialog::getOpenFileName(this, tr("Rechnung öffnen"),
            settings.value("directory").toString(), tr("Rechnungen (*.pdf *.png *.jpg *.jpeg)"));
        openFile(chosen);
    }

    void loaded()
    {
        document = watcher.result();
        const auto details = QJsonDocument::fromJson(takeBytes(giro_details(document.get()))).object();
        if (details.contains("error")) {
            notice->setText(tr("Die Rechnung konnte nicht geöffnet werden. %1").arg(details["error"].toString()));
            refreshControls();
            return;
        }
        count = details["pages"].toInt();
        const auto values = details["fields"].toArray();
        for (std::size_t i = 0; i < fields.size(); ++i) {
            fields[i]->setText(values[qsizetype(i)].toString());
            if ((i == 0 || i == 1 || i == 3) && fields[i]->text().isEmpty()) {
                hints[i]->setText(tr("Bitte aus der Rechnung übernehmen."));
                hints[i]->show();
            }
        }
        const auto accounts = details["accounts"].toArray();
        ibanChoices->clear();
        if (accounts.size() > 1) {
            ibanChoices->setPlaceholderText(tr("Gefundene IBAN auswählen"));
            for (const auto &account : accounts)
                ibanChoices->addItem(account.toString());
            ibanChoices->setCurrentIndex(-1);
            ibanChoices->show();
        }
        notice->setText(details["externalPayment"].toBool()
            ? tr("Die Rechnung verweist auf eine separate Zahlungsanweisung. Bitte prüfe den Zahlungsempfänger.")
            : tr("Vergleiche die erkannten Zahlungsdaten mit der Rechnung."));
        setWindowTitle(tr("%1 — %2").arg(filename, appName));
        showPage(0);
    }

    void showPage(int page)
    {
        if (page < 0 || page >= count)
            return;
        index = page;
        preview->showPage(document, static_cast<std::size_t>(page));
        refreshControls();
    }

    void refreshControls()
    {
        openAction->setEnabled(!watcher.isRunning());
        previous->setEnabled(count > 0 && index > 0);
        next->setEnabled(index + 1 < count);
        pageLabel->setText(count > 0 ? tr("Seite %1 von %2").arg(index + 1).arg(count) : QString());
        for (auto *action : {zoomIn, zoomOut, fit})
            action->setEnabled(count > 0);
        for (auto *field : fields)
            field->setEnabled(count > 0);
        generate->setEnabled(count > 0);
    }

    void clearQr()
    {
        if (qrLabel) {
            qrLabel->clear();
            qrLabel->hide();
        }
    }

    void generateCode()
    {
        QJsonArray values;
        for (const auto *field : fields)
            values.append(field->text());
        const auto encoded = QJsonDocument(values).toJson(QJsonDocument::Compact);
        const auto result = takeBytes(giro_code(encoded.data(), encoded.size()));
        const auto image = QImage::fromData(result);
        if (image.isNull()) {
            const QString error = QJsonDocument::fromJson(result).object()["error"].toString();
            auto i = error.startsWith("IBAN") ? 1 : error.startsWith("BIC") ? 2
                       : error.contains("Betrag") ? 3 : error.startsWith("Verwendungszweck") ? 4 : 0;
            hints[i]->setText(error);
            hints[i]->show();
            fields[i]->setFocus();
            return;
        }
        const int scale = qMax(1, 224 / image.width());
        qrLabel->setPixmap(QPixmap::fromImage(image.scaled(image.size() * scale, Qt::IgnoreAspectRatio, Qt::FastTransformation)));
        qrLabel->show();
        qobject_cast<QScrollArea *>(settingsSplitter->widget(1))->ensureWidgetVisible(qrLabel);
    }

    QSettings settings;
    QFutureWatcher<Document> watcher;
    Document document;
    QString filename;
    int index = 0;
    int count = 0;
    Preview *preview;
    QSplitter *settingsSplitter;
    QAction *openAction, *previous, *next, *zoomIn, *zoomOut, *fit;
    QLabel *pageLabel, *notice, *qrLabel = nullptr;
    QComboBox *ibanChoices;
    QPushButton *generate;
    std::array<QLineEdit *, 5> fields;
    std::array<QLabel *, 5> hints;
};

#ifndef GIRO_UI_TEST
int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    app.setOrganizationName("SEPA-GiroCode");
    app.setApplicationName(appName);
    app.setDesktopFileName(appId);
    Window window;
    window.show();
    if (app.arguments().size() == 2)
        window.openFile(app.arguments().at(1));
    return app.exec();
}
#endif
