from rich.console import Console
from rich.text import Text
import pyfiglet
import sys
import os

# Monokai-ish colors
COLORS = ["#F92672", "#A6E22E", "#66D9EF", "#AE81FF", "#FD971F"]

console = Console(force_terminal=True, color_system="truecolor", width=100)
font = "slant"
text_str = "ThinThread"
ascii_art = pyfiglet.figlet_format(text_str, font=font)
lines = ascii_art.splitlines()

output_dir = "codex-rs/tui2/frames/council"
# Reuse the existing directory so we overwrite the "Codex Council" frames
os.makedirs(output_dir, exist_ok=True)

for frame_idx in range(1, 37):
    styled_text = Text()
    for i, line in enumerate(lines):
        # Cycle colors per line
        color_idx = (i + frame_idx) % len(COLORS)
        color = COLORS[color_idx]
        styled_text.append(line + "\n", style=color) 
    
    with console.capture() as capture:
        console.print(styled_text)
    
    output = capture.get()
    
    filename = os.path.join(output_dir, f"frame_{frame_idx}.txt")
    with open(filename, "w") as f:
        f.write(output)

print("Generated ThinThread frames.")
