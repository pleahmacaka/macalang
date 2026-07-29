#include <stdio.h>
#include <stdlib.h>
int main(void){ long n=400; long*a=malloc(8*n*n),*b=malloc(8*n*n),*c=malloc(8*n*n);
  for(long i=0;i<n;i++) for(long j=0;j<n;j++){ a[i*n+j]=(i+j)%100; b[i*n+j]=(i*j)%100; }
  for(long i=0;i<n;i++) for(long j=0;j<n;j++){ long s=0; for(long k=0;k<n;k++) s+=a[i*n+k]*b[k*n+j]; c[i*n+j]=s; }
  long sum=0; for(long i=0;i<n*n;i++) sum=(sum+c[i])%1000000007;
  printf("%ld\n",sum); return 0; }
