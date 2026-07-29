package main
import "fmt"
func matmul(n int) int64 { a:=make([]int64,n*n); b:=make([]int64,n*n); c:=make([]int64,n*n)
  for i:=0;i<n;i++ { for j:=0;j<n;j++ { a[i*n+j]=int64((i+j)%100); b[i*n+j]=int64((i*j)%100) } }
  for i:=0;i<n;i++ { for j:=0;j<n;j++ { var s int64=0; for k:=0;k<n;k++ { s+=a[i*n+k]*b[k*n+j] }; c[i*n+j]=s } }
  var sum int64=0; for i:=0;i<n*n;i++ { sum=(sum+c[i])%1000000007 }; return sum }
func main(){ fmt.Println(matmul(400)) }
