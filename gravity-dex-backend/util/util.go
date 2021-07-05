package util

import "time"

func NewImmediateTicker(d time.Duration) *time.Ticker {
	t := time.NewTicker(d)
	oc := t.C
	nc := make(chan time.Time, 1)
	go func() {
		nc <- time.Now()
		for tm := range oc {
			nc <- tm
		}
	}()
	t.C = nc
	return t
}

func StringInSlice(s string, ss []string) bool {
	for _, x := range ss {
		if s == x {
			return true
		}
	}
	return false
}

func MinInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}
