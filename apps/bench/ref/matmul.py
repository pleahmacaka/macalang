def matmul(n):
    a=[(i//n + i%n)%100 for i in range(n*n)]
    b=[((i//n)*(i%n))%100 for i in range(n*n)]
    c=[0]*(n*n)
    for i in range(n):
        for j in range(n):
            s=0
            for k in range(n): s+=a[i*n+k]*b[k*n+j]
            c[i*n+j]=s
    return sum(c)%1000000007
print(matmul(400))
