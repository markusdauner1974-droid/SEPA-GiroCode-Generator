import queue
import threading
import tkinter as tk
from pathlib import Path
from tkinter import filedialog, messagebox, ttk

from PIL import Image, ImageTk

from beleg import read_document
from zahlung import payload, qr_image, suggestions


class Application:
    def __init__(self, root):
        self.root = root
        root.title('Rechnung zu GiroCode')
        root.geometry('1080x740')
        root.minsize(750, 510)
        self.pages = []
        self.index = 0
        self.zoom = 1
        self.results = queue.Queue()
        toolbar = ttk.Frame(root, padding=8)
        toolbar.pack(fill='x')
        self.opener = ttk.Button(toolbar, text='Rechnung öffnen …', command=self.open)
        self.opener.pack(side='left')
        self.status = tk.StringVar(value='PDF, JPG oder PNG auswählen.')
        ttk.Label(toolbar, textvariable=self.status).pack(side='left', padx=12)
        pane = ttk.Panedwindow(root, orient='horizontal')
        pane.pack(fill='both', expand=True)
        left = ttk.Frame(pane)
        pane.add(left, weight=3)
        right = ttk.Frame(pane, padding=16)
        pane.add(right, weight=2)
        nav = ttk.Frame(left)
        nav.pack(fill='x')
        for label, action in [('‹', lambda: self.move(-1)), ('›', lambda: self.move(1)),
                              ('−', lambda: self.resize(-0.5)), ('+', lambda: self.resize(0.5))]:
            ttk.Button(nav, text=label, width=4, command=action).pack(side='left')
        self.counter = ttk.Label(nav)
        self.counter.pack(side='left', padx=8)
        frame = ttk.Frame(left)
        frame.pack(fill='both', expand=True)
        self.canvas = tk.Canvas(frame, background='#ddd', highlightthickness=0)
        self.canvas.grid(row=0, column=0, sticky='nsew')
        frame.rowconfigure(0, weight=1)
        frame.columnconfigure(0, weight=1)
        for axis, row, column, sticky in [('x', 1, 0, 'ew'), ('y', 0, 1, 'ns')]:
            bar = ttk.Scrollbar(frame, orient='horizontal' if axis == 'x' else 'vertical',
                                command=getattr(self.canvas, axis + 'view'))
            self.canvas.configure(**{axis + 'scrollcommand': bar.set})
            bar.grid(row=row, column=column, sticky=sticky)
        self.canvas.bind('<Configure>', lambda event: self.draw())
        self.fields = []
        for label in ['Empfänger', 'IBAN', 'BIC', 'Betrag in Euro', 'Verwendungszweck']:
            ttk.Label(right, text=label).pack(anchor='w')
            value = tk.StringVar()
            self.fields.append(value)
            if label == 'IBAN':
                entry = self.accounts = ttk.Combobox(right, textvariable=value)
            else:
                entry = ttk.Entry(right, textvariable=value)
            entry.pack(fill='x', pady=(2, 10))
        self.notice = ttk.Label(right, wraplength=320, text='Zahlungsdaten mit der Rechnung abgleichen.')
        self.notice.pack(anchor='w', pady=6)
        self.generate = ttk.Button(right, text='GiroCode erstellen', command=self.show_code, state='disabled')
        self.generate.pack(anchor='w', pady=8)

    def move(self, step):
        self.index = min(max(0, self.index + step), max(0, len(self.pages) - 1))
        self.canvas.yview_moveto(0)
        self.draw()

    def resize(self, step):
        self.zoom = min(4, max(1, self.zoom + step))
        self.draw()

    def draw(self):
        self.canvas.delete('all')
        if self.pages:
            page = self.pages[self.index]
            scale = self.zoom * max(1, self.canvas.winfo_width() - 8) / page.width
            size = tuple(max(1, round(s * scale)) for s in page.size)
            self.preview = ImageTk.PhotoImage(page.resize(size, Image.Resampling.LANCZOS))
            self.canvas.create_image(0, 0, anchor='nw', image=self.preview)
            self.canvas.configure(scrollregion=(0, 0, *size))
            self.counter.configure(text=f'Seite {self.index + 1} von {len(self.pages)}')

    def open(self):
        path = filedialog.askopenfilename(filetypes=[('Rechnungen', '*.pdf *.PDF *.jpg *.JPG *.jpeg *.JPEG *.png *.PNG'), ('Alle Dateien', '*')])
        if not path:
            return
        self.pages = []
        self.index, self.zoom = 0, 1
        self.draw()
        self.canvas.yview_moveto(0)
        self.canvas.xview_moveto(0)
        self.counter.configure(text='')
        self.accounts.configure(values=())
        for field in self.fields:
            field.set('')
        self.opener.configure(state='disabled')
        self.generate.configure(state='disabled')
        self.status.set('Rechnung wird lokal gelesen …')

        def worker():
            try:
                result = read_document(path)
            except Exception as exc:
                result = exc
            self.results.put(result)

        threading.Thread(target=worker, daemon=True).start()
        self.root.after(120, self.receive, Path(path).name)

    def receive(self, name):
        try:
            result = self.results.get_nowait()
        except queue.Empty:
            self.root.after(120, self.receive, name)
            return
        self.opener.configure(state='normal')
        if isinstance(result, Exception):
            self.status.set('Einlesen fehlgeschlagen.')
            messagebox.showerror('Rechnung öffnen', str(result))
            return
        self.pages, text = result
        values, accounts = suggestions(text)
        self.accounts.configure(values=accounts)
        for field, value in zip(self.fields, values):
            field.set(value)
        hint = 'Mehrere IBANs erkannt. Bitte die passende auswählen und die zugehörige BIC prüfen.' if len(accounts) > 1 else 'Bitte alle Angaben am Beleg prüfen und fehlende Felder ergänzen.'
        self.notice.configure(text=hint)
        self.status.set(name)
        self.draw()
        self.generate.configure(state='normal')

    def show_code(self):
        try:
            image = qr_image(payload(*(field.get() for field in self.fields)))
        except ValueError as exc:
            messagebox.showwarning('Zahlungsdaten prüfen', str(exc))
            return
        dialog = tk.Toplevel(self.root)
        dialog.title('Mit der Banking-App scannen')
        photo = ImageTk.PhotoImage(image)
        qr = ttk.Label(dialog, image=photo)
        qr.image = photo
        qr.pack(padx=10, pady=10)
        ttk.Button(dialog, text='Schließen', command=dialog.destroy).pack(pady=(0, 12))
        dialog.transient(self.root)
        dialog.grab_set()


if __name__ == '__main__':
    window = tk.Tk()
    Application(window)
    window.mainloop()
