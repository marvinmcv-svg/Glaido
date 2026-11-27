"""A focused text box that writes its contents to a file, so E2E can assert what was pasted."""
import sys, tkinter as tk

out = sys.argv[1]
root = tk.Tk()
root.title("E2E target")
root.geometry("600x80+40+40")
entry = tk.Entry(root, font=("DejaVu Sans", 14))
entry.pack(fill="x", padx=10, pady=20)

def tick():
    with open(out, "w") as f:
        f.write(entry.get())
    root.after(100, tick)

def focus():
    root.focus_force()
    entry.focus_set()

root.after(300, focus)
tick()
root.mainloop()
