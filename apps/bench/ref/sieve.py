import sys
def count_primes(n):
    m=bytearray(n+1); c=0
    for i in range(2,n+1):
        if not m[i]:
            c+=1; m[i*i::i]=b'\1'*len(m[i*i::i])
    return c
print(count_primes(10000000))
