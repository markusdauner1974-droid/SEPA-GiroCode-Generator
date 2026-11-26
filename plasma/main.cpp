#include "preview.h"

#include <QAction>
#include <QApplication>
#include <QComboBox>
#include <QDragEnterEvent>
#include <QDragMoveEvent>
#include <QDesktopServices>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QDirIterator>
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
#include <QMessageBox>
#include <QMimeData>
#include <QPushButton>
#include <QPlainTextEdit>
#include <QTabWidget>
#include <QScrollArea>
#include <QSettings>
#include <QSplitter>
#include <QStringList>
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

QString displayIban(const QString &value)
{
    QString result;
    for (const QChar ch : value) {
        if (ch.isSpace()) continue;
        const qsizetype digits = result.size() - result.count(QLatin1Char(' '));
        if (digits > 0 && digits % 4 == 0) result.append(QLatin1Char(' '));
        result.append(ch);
    }
    return result;
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
        auto *help = menuBar()->addMenu(tr("&Hilfe"));
        help->setObjectName(QStringLiteral("helpMenu"));
        auto *instructions = help->addAction(QIcon::fromTheme("help-contents"), tr("&Hilfe zum Programm"),
                                             QKeySequence::HelpContents, this, [this] {
            QMessageBox::information(this, tr("Hilfe zum Programm"),
                tr("1. Öffne eine PDF-Rechnung oder ein Foto.\n"
                   "2. Vergleiche die erkannten Zahlungsdaten mit der Rechnung und korrigiere sie bei Bedarf.\n"
                   "3. Erstelle den GiroCode und scanne ihn mit deiner Banking-App.\n\n"
                   "Bei einem Scan oder einer fehlerhaften Textebene kannst du unter Datei den PDF-Text per OCR neu erkennen."));
        });
        instructions->setObjectName(QStringLiteral("helpInstructions"));
        auto *report = help->addAction(QIcon::fromTheme("tools-report-bug"), tr("&Probleme melden …"), this, [this] {
            const QUrl url(QStringLiteral("https://github.com/markusdauner1974-droid/SEPA-GiroCode-Generator/issues"));
            if (!QDesktopServices::openUrl(url))
                QMessageBox::warning(this, tr("Browser konnte nicht geöffnet werden"),
                    tr("Bitte öffne diese Adresse im Browser:\n%1").arg(url.toString()));
        });
        report->setObjectName(QStringLiteral("reportProblem"));
        auto *projectQr = help->addAction(QIcon::fromTheme("view-barcode-qr"), tr("Projektseite als QR-Code"), this, [this] {
            const QPixmap code = QIcon::fromTheme(appId + QStringLiteral(".ProjectQR")).pixmap(384, 384);
            if (code.isNull()) {
                QMessageBox::warning(this, tr("QR-Code nicht verfügbar"),
                    tr("Die Projektseite kann unter https://github.com/markusdauner1974-droid/SEPA-GiroCode-Generator geöffnet werden."));
                return;
            }
            QMessageBox box(this);
            box.setWindowTitle(tr("Projektseite auf GitHub"));
            box.setText(tr("Scanne den QR-Code, um die Projektseite zu öffnen."));
            box.setIconPixmap(code);
            box.exec();
        });
        projectQr->setObjectName(QStringLiteral("projectQr"));
        auto *licenses = help->addAction(QIcon::fromTheme("text-x-generic"), tr("Open-Source-Lizenzen"), this, [this] {
            const QString root = QCoreApplication::applicationDirPath() + QStringLiteral("/../share/licenses");
            auto readFile = [](const QString &path) {
                QFile file(path);
                if (!file.open(QIODevice::ReadOnly | QIODevice::Text))
                    return QObject::tr("Datei nicht verfügbar: %1").arg(path);
                return QString::fromUtf8(file.readAll());
            };
            QDialog dialog(this);
            dialog.setWindowTitle(tr("Open-Source-Lizenzen"));
            dialog.resize(850, 650);
            auto *layout = new QVBoxLayout(&dialog);
            auto *tabs = new QTabWidget(&dialog);
            auto addTab = [&](const QString &title, const QString &text) {
                auto *view = new QPlainTextEdit(&dialog);
                view->setReadOnly(true);
                view->setPlainText(text);
                tabs->addTab(view, title);
            };
            const QString appDir = root + QStringLiteral("/sepa-girocode-generator/");
            addTab(tr("Übersicht"), readFile(appDir + QStringLiteral("THIRD-PARTY-NOTICES.txt")));
            addTab(tr("Rust-Abhängigkeiten"), readFile(appDir + QStringLiteral("rust-dependencies-and-licenses.txt")));
            QString native;
            const QStringList dirs = {QStringLiteral("poppler"), QStringLiteral("poppler-data"),
                                      QStringLiteral("tesseract"), QStringLiteral("tessdata_fast"),
                                      QStringLiteral("leptonica")};
            for (const auto &dir : dirs) {
                QDirIterator it(root + QLatin1Char('/') + dir, QDir::Files, QDirIterator::Subdirectories);
                while (it.hasNext()) {
                    const QString path = it.next();
                    native += QStringLiteral("\n=== %1 ===\n\n").arg(dir + QLatin1Char('/') + QFileInfo(path).fileName());
                    native += readFile(path);
                    native += QLatin1Char('\n');
                }
            }
            for (const auto &file : {QStringLiteral("LICENSE"), QStringLiteral("LICENSE-MIT"), QStringLiteral("DejaVu-font-license.txt")}) {
                native += QStringLiteral("\n=== %1 ===\n\n").arg(file);
                native += readFile(appDir + file);
            }
            if (native.isEmpty()) native = tr("Keine mitgelieferten Lizenzdateien gefunden.");
            addTab(tr("Mitgelieferte Lizenztexte"), native);
            // The KDE runtime is installed separately. Inspect only its own
            // manifest and licence directory; do not claim a complete SBOM.
            auto *runtime = new QPlainTextEdit(&dialog);
            runtime->setReadOnly(true);
            tabs->addTab(runtime, tr("KDE-Laufzeit"));
            connect(tabs, &QTabWidget::currentChanged, &dialog, [runtime, tabs, readFile](int index) {
                if (tabs->widget(index) != runtime || runtime->property("loaded").toBool()) return;
                runtime->setProperty("loaded", true);
                QString text = QObject::tr("Die KDE-Flatpak-Laufzeit wird separat bereitgestellt. Die folgenden Daten stammen aus ihrer installierten Manifest- und Lizenzsammlung; Umfang und Aktualität hängen von der Laufzeit ab.\n\n");
                const QString manifest = QStringLiteral("/usr/manifest.json");
                text += QObject::tr("=== Laufzeitmanifest ===\n\n") + readFile(manifest);
                const QString runtimeLicenses = QStringLiteral("/usr/share/licenses/org.kde.Platform");
                if (!QDir(runtimeLicenses).exists()) {
                    text += QObject::tr("\n\nKein KDE-Lizenzverzeichnis unter %1 gefunden.\n").arg(runtimeLicenses);
                } else {
                    QDirIterator it(runtimeLicenses, QDir::Files | QDir::Readable, QDirIterator::Subdirectories);
                    qsizetype total = 0;
                    int files = 0;
                    constexpr qsizetype maxBytes = 12 * 1024 * 1024;
                    while (it.hasNext()) {
                        const QString path = it.next();
                        if (QFileInfo(path).size() > 1024 * 1024 || total >= maxBytes) {
                            text += QObject::tr("\nEinige Laufzeitdateien wurden aus Größen- oder Platzgründen nicht angezeigt. Pfad: %1\n").arg(runtimeLicenses);
                            break;
                        }
                        const QString content = readFile(path);
                        total += content.size() * qsizetype(sizeof(QChar));
                        text += QStringLiteral("\n\n=== %1 ===\n\n").arg(QDir(runtimeLicenses).relativeFilePath(path)) + content;
                        ++files;
                    }
                    if (files == 0) text += QObject::tr("\nKeine lesbaren Lizenzdateien im KDE-Laufzeitverzeichnis gefunden.\n");
                }
                runtime->setPlainText(text);
            });
            layout->addWidget(tabs);
            auto *buttons = new QDialogButtonBox(QDialogButtonBox::Close, &dialog);
            connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
            layout->addWidget(buttons);
            dialog.exec();
        });
        licenses->setObjectName(QStringLiteral("openSourceLicenses"));
        help->addSeparator();
        auto *about = help->addAction(QIcon::fromTheme("help-about"), tr("&Über SEPA-GiroCode-Generator"), this, [this] {
            QMessageBox::about(this, tr("Über SEPA-GiroCode-Generator"),
                tr("SEPA-GiroCode-Generator\n\nErstellt GiroCodes aus Rechnungen.\n\n"
                   "Lizenz: GNU General Public License, Version 3.0 (GPL-3.0-only).\n\n"
                   "Idee und Konzept: Markus Dauner\n"
                   "Programmierung: mit Unterstützung von OpenAI ChatGPT."));
        });
        about->setObjectName(QStringLiteral("aboutProgram"));
        auto *toolbar = addToolBar(tr("Rechnung"));
        toolbar->setObjectName("invoiceTools");
        toolbar->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        openAction = file->addAction(QIcon::fromTheme("document-open"), tr("&Öffnen …"), QKeySequence::Open,
                                     this, &Window::openDialog);
        toolbar->addAction(openAction);
        rereadAction = file->addAction(QIcon::fromTheme("view-refresh"), tr("PDF-Text per OCR neu erkennen"),
                                       this, [this] { openFile(currentPath, true); });
        rereadAction->setStatusTip(tr("Liest das Seitenbild und übernimmt die neu erkannten Felder."));
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
        preview->viewport()->setAcceptDrops(true);
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
        references = plainLabel();
        references->setTextInteractionFlags(Qt::TextSelectableByMouse);
        references->hide();
        form->addWidget(references);
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
        qApp->installEventFilter(this);
        refreshControls();
    }

    void openFile(const QString &path, bool forceOcr = false)
    {
        if (watcher.isRunning() || path.isEmpty())
            return;
        settings.setValue("directory", QFileInfo(path).absolutePath());
        currentPath = path;
        filename = QFileInfo(path).fileName();
        notice->setText(forceOcr ? tr("PDF-Text wird per OCR neu erkannt …") : tr("Rechnung wird gelesen …"));
        references->hide();
        references->clear();
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
        watcher.setFuture(QtConcurrent::run([encoded, forceOcr] {
            return Document(giro_open(encoded.data(), encoded.size(), forceOcr), giro_close);
        }));
        refreshControls();
    }

protected:
    // Child widgets such as the preview viewport and line edits may accept
    // drops themselves. Intercept invoice files before they consume the event,
    // while leaving ordinary text drags and other windows untouched.
    bool eventFilter(QObject *object, QEvent *event) override
    {
        auto *widget = qobject_cast<QWidget *>(object);
        if (widget && (widget == this || isAncestorOf(widget)) && widget->window() == this &&
            (event->type() == QEvent::DragEnter || event->type() == QEvent::DragMove || event->type() == QEvent::Drop)) {
            auto *drop = static_cast<QDropEvent *>(event);
            const auto urls = drop->mimeData()->urls();
            if (urls.size() == 1 && supported(urls.front())) {
                if (watcher.isRunning()) {
                    drop->ignore();
                    return true;
                }
                drop->acceptProposedAction();
                if (event->type() == QEvent::Drop)
                    openFile(urls.front().toLocalFile());
                return true;
            }
        }
        return QMainWindow::eventFilter(object, event);
    }

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
            const QString value = values[qsizetype(i)].toString();
            fields[i]->setText(i == 1 ? displayIban(value) : value);
            if ((i == 0 || i == 1 || i == 3) && fields[i]->text().isEmpty()) {
                hints[i]->setText(tr("Bitte aus der Rechnung übernehmen."));
                hints[i]->show();
            }
        }
        QStringList numbers;
        const QString issuer = details["invoiceIssuer"].toString();
        if (!issuer.isEmpty() && issuer != fields[0]->text())
            numbers.append(tr("Rechnungsaussteller: %1").arg(issuer));
        const QString invoiceNumber = details["invoiceNumber"].toString();
        const QString customerNumber = details["customerNumber"].toString();
        if (!invoiceNumber.isEmpty())
            numbers.append(tr("Rechnungsnummer: %1").arg(invoiceNumber));
        if (!customerNumber.isEmpty())
            numbers.append(tr("Kundennummer: %1").arg(customerNumber));
        references->setText(numbers.join(QLatin1Char('\n')));
        references->setVisible(!numbers.isEmpty());
        if (details["returnAddressName"].toBool()) {
            hints[0]->setText(tr("Aus der Absenderzeile übernommen. Bitte mit der Rechnung abgleichen."));
            hints[0]->show();
        }
        const auto accounts = details["accounts"].toArray();
        ibanChoices->clear();
        if (accounts.size() > 1) {
            ibanChoices->setPlaceholderText(tr("Gefundene IBAN auswählen"));
            for (const auto &account : accounts)
                ibanChoices->addItem(displayIban(account.toString()));
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
        rereadAction->setEnabled(!watcher.isRunning()
            && QFileInfo(currentPath).suffix().compare("pdf", Qt::CaseInsensitive) == 0);
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
    QString filename, currentPath;
    int index = 0;
    int count = 0;
    Preview *preview;
    QSplitter *settingsSplitter;
    QAction *openAction, *rereadAction, *previous, *next, *zoomIn, *zoomOut, *fit;
    QLabel *pageLabel, *notice, *references, *qrLabel = nullptr;
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
    app.setWindowIcon(QIcon::fromTheme(appId));
    Window window;
    window.show();
    if (app.arguments().size() == 2)
        window.openFile(app.arguments().at(1));
    return app.exec();
}
#endif
