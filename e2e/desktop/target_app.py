"""A focused text box that writes its contents to a file, so E2E can assert what was pasted."""
import os, sys, tkinter as tk

out = sys.argv[1]
title = sys.argv[2] if len(sys.argv) > 2 else "E2E target"
y = int(sys.argv[3]) if len(sys.argv) > 3 else 40
root = tk.Tk()
root.title(title)
root.geometry(f"600x80+40+{y}")
entry = tk.Entry(root, font=("DejaVu Sans", 14))
entry.pack(fill="x", padx=10, pady=20)

def tick():
    # Write-then-rename, so a reader never sees a truncated, empty file.
    with open(out + ".tmp", "w") as f:
        f.write(entry.get())
    os.replace(out + ".tmp", out)
    root.after(100, tick)

def focus():
    root.focus_force()
    entry.focus_set()

if len(sys.argv) > 4 and sys.argv[4] == "nofocus":
    entry.focus_set()  # keys go to the entry whenever this window gets focus
else:
    root.after(300, focus)
tick()
root.mainloop()
