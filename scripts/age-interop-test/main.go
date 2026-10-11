// Isolated test oracle: reference age v1.2.1. Credentials enter only via stdin.
package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"encoding/json"
	"filippo.io/age"
	"fmt"
	"io"
	"os"
)

func check(e error) {
	if e != nil {
		panic("age interoperability check failed")
	}
}
func main() {
	var p struct{ Mode, File, Password string }
	check(json.NewDecoder(os.Stdin).Decode(&p))
	if p.Mode == "decrypt" {
		f, e := os.Open(p.File)
		check(e)
		defer f.Close()
		id, e := age.NewScryptIdentity(p.Password)
		check(e)
		id.SetMaxWorkFactor(18)
		r, e := age.Decrypt(f, id)
		check(e)
		data, e := io.ReadAll(io.LimitReader(r, 1024*1024))
		check(e)
		if len(data) == 1024*1024 {
			panic("fixture too large")
		}
		g, e := gzip.NewReader(bytes.NewReader(data))
		check(e)
		t := tar.NewReader(g)
		found := false
		for {
			h, e := t.Next()
			if e == io.EOF {
				break
			}
			check(e)
			if h.Typeflag == tar.TypeReg {
				b, e := io.ReadAll(t)
				check(e)
				if string(b) == "encrypted fixture contents\n" {
					found = true
				}
			}
		}
		if !found {
			panic("fixture mismatch")
		}
		fmt.Println("PASS reference Go decrypt")
	} else {
		f, e := os.OpenFile(p.File, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
		check(e)
		defer f.Close()
		r, e := age.NewScryptRecipient(p.Password)
		check(e)
		r.SetWorkFactor(18)
		w, e := age.Encrypt(f, r)
		check(e)
		g := gzip.NewWriter(w)
		t := tar.NewWriter(g)
		b := []byte("reference age fixture\n")
		check(t.WriteHeader(&tar.Header{Name: "reference.txt", Mode: 0600, Size: int64(len(b))}))
		_, e = t.Write(b)
		check(e)
		check(t.Close())
		check(g.Close())
		check(w.Close())
		fmt.Println("PASS reference Go encrypt")
	}
}
