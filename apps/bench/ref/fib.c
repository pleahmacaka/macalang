#include <stdio.h>
long fib(long n){ return n<2?n:fib(n-1)+fib(n-2); }
int main(){ printf("%ld\n", fib(40)); return 0; }
