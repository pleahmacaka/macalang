import sys
sys.setrecursionlimit(1000000)
def tak(x,y,z): return tak(tak(x-1,y,z),tak(y-1,z,x),tak(z-1,x,y)) if y<x else z
print(tak(32,16,8))
