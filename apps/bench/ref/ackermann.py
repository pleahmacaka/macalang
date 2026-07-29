import sys
sys.setrecursionlimit(1000000)
def ack(m,n): return n+1 if m==0 else (ack(m-1,1) if n==0 else ack(m-1,ack(m,n-1)))
print(ack(3,10))
