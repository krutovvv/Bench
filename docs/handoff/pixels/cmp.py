import sys, os
from PIL import Image, ImageChops
# cmp.py after-dir: a pixel is a real change when it differs from both before passes.
W=os.path.dirname(os.path.abspath(__file__))
aft=os.path.join(W, sys.argv[1])
for n in sorted(os.listdir(aft)):
    if not n.endswith('.png'): continue
    y = Image.open(os.path.join(aft,n)).convert('RGB')
    masks=[]
    for b in ('before','before2'):
        p=os.path.join(W,b,n)
        if not os.path.exists(p): continue
        x=Image.open(p).convert('RGB')
        if x.size!=y.size: masks.append(None); continue
        masks.append(ImageChops.difference(x,y).convert('L').point(lambda v:255 if v else 0))
    if any(m is None for m in masks): print(n,'SIZE'); continue
    m=masks[0]
    for o in masks[1:]: m=ImageChops.multiply(m,o)
    from PIL import ImageDraw
    d=ImageDraw.Draw(m)
    # Noise: traffic lights, the sidebar list's fade over its foot, the overlay scrollbars.
    for box in [(0,0,170,70),(0,1095,390,1140),(2340,0,2482,1600)]: d.rectangle(box,fill=0)
    bb=m.getbbox()
    cnt=sum(1 for v in m.get_flattened_data() if v) if bb else 0
    print(n, 'same' if bb is None else f'DIFF {bb} px={cnt}')
