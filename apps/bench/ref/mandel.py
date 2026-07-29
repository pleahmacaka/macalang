def mandel(size):
    total=0
    for py in range(size):
        for px in range(size):
            x0=px/size*3.5-2.5; y0=py/size*2.0-1.0; x=0.0; y=0.0; it=0
            while it<1000 and x*x+y*y<=4.0:
                x,y=x*x-y*y+x0, 2*x*y+y0; it+=1
            total+=it
    return total
print(mandel(800))
