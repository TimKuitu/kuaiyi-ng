import struct, os

src = r'C:\Users\Lenovo\.zcode\workspace\default\icons'
out = r'D:\Lenovo\Desktop\ToolBox\快译\快译NG\res\kuaiyi.ico'

sizes = [256, 64, 48, 32, 16]
blobs = []
for s in sizes:
    blobs.append((s, open(os.path.join(src, f'icon_{s}.png'), 'rb').read()))

# ICONDIR
data = struct.pack('<HHH', 0, 1, len(blobs))
# ICONDIRENTRY: w, h, colors, reserved, planes, bitcount, bytes, offset
offset = 6 + 16 * len(blobs)
entries = b''
for s, blob in blobs:
    w = 0 if s == 256 else s
    h = 0 if s == 256 else s
    entries += struct.pack('<BBBBHHII', w, h, 0, 0, 1, 32, len(blob), offset)
    offset += len(blob)

with open(out, 'wb') as f:
    f.write(data + entries + b''.join(b for _, b in blobs))
print('ICO written:', out, os.path.getsize(out), 'bytes')
