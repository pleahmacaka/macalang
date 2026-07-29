#include <stdio.h>
long ack(long m,long n){ return m==0 ? n+1 : (n==0 ? ack(m-1,1) : ack(m-1,ack(m,n-1))); }
int main(){ printf("%ld\n", ack(3,10)); return 0; }
