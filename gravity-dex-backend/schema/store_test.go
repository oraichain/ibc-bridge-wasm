package schema

import (
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMergeVolumes(t *testing.T) {
	v1 := Volumes{
		time.Date(2021, time.April, 30, 6, 0, 35, 0, time.UTC).Unix(): CoinMap{
			"atom": 100,
		},
		time.Date(2021, time.April, 30, 6, 0, 42, 0, time.UTC).Unix(): CoinMap{
			"atom": 200,
		},
		time.Date(2021, time.April, 30, 6, 1, 0, 0, time.UTC).Unix(): CoinMap{
			"atom": 50,
			"usd":  20,
		},
	}
	v2 := Volumes{
		time.Date(2021, time.April, 30, 6, 0, 37, 0, time.UTC).Unix(): CoinMap{
			"atom": 50,
		},
		time.Date(2021, time.April, 30, 6, 1, 30, 0, time.UTC).Unix(): CoinMap{
			"usd": 70,
		},
	}
	v := MergeVolumes(v1, v2)
	t1 := time.Date(2021, time.April, 30, 6, 0, 0, 0, time.UTC).Truncate(VolumeTimeUnit).Unix()
	t2 := time.Date(2021, time.April, 30, 6, 1, 0, 0, time.UTC).Truncate(VolumeTimeUnit).Unix()
	assert.Equal(t, int64(350), v[t1]["atom"])
	assert.Equal(t, int64(0), v[t1]["usd"])
	assert.Equal(t, int64(50), v[t2]["atom"])
	assert.Equal(t, int64(90), v[t2]["usd"])
}

func TestMergeNilVolumes(t *testing.T) {
	v := MergeVolumes(nil, nil)
	require.NotNil(t, v)
	require.Len(t, v, 0)
}

func TestVolumes_RemoveOutdated(t *testing.T) {
	v := Volumes{
		time.Date(2021, time.April, 30, 6, 0, 0, 0, time.UTC).Truncate(VolumeTimeUnit).Unix(): CoinMap{
			"atom": 20,
			"usd":  100,
		},
		time.Date(2021, time.April, 30, 6, 1, 30, 0, time.UTC).Truncate(VolumeTimeUnit).Unix(): CoinMap{
			"atom": 100,
			"usd":  200,
		},
		time.Date(2021, time.April, 30, 7, 0, 0, 0, time.UTC).Truncate(VolumeTimeUnit).Unix(): CoinMap{
			"usd": 300,
		},
	}
	v.RemoveOutdated(time.Date(2021, time.April, 30, 7, 2, 0, 0, time.UTC).Add(-time.Hour))
	require.Len(t, v, 1)
}
