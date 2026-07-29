package main
import "fmt"
func countPrimes(n int) int64 { m:=make([]byte,n+1); var c int64=0
  for i:=2;i<=n;i++ { if m[i]==0 { c++; for j:=i+i;j<=n;j+=i { m[j]=1 } } }; return c }
func main(){ fmt.Println(countPrimes(10000000)) }
