package schema

import "time"

type GetStatusResponse struct {
	LatestBlockHeight int64 `json:"latestBlockHeight"`
}

type GetScoreBoardRequest struct {
	Address string `query:"address"`
}

type GetScoreBoardResponse struct {
	BlockHeight int64                          `json:"blockHeight"`
	Me          *GetScoreBoardResponseAccount  `json:"me"`
	Accounts    []GetScoreBoardResponseAccount `json:"accounts"`
	UpdatedAt   time.Time                      `json:"updatedAt"`
}

type GetScoreBoardResponseAccount struct {
	Ranking      int     `json:"ranking"`
	Username     string  `json:"username"`
	Address      string  `json:"address"`
	TotalScore   float64 `json:"totalScore"`
	TradingScore float64 `json:"tradingScore"`
	ActionScore  float64 `json:"actionScore"`
	IsValid      bool    `json:"isValid"`
}

type SearchAccountRequest struct {
	Query string `query:"q"`
}

type SearchAccountResponse struct {
	BlockHeight int64                         `json:"blockHeight"`
	Account     *GetScoreBoardResponseAccount `json:"account"`
	UpdatedAt   time.Time                     `json:"updatedAt"`
}

type GetActionStatusRequest struct {
	Address string `query:"address"`
}

type GetActionStatusResponse struct {
	BlockHeight int64                           `json:"blockHeight"`
	Account     *GetActionStatusResponseAccount `json:"account"`
	UpdatedAt   time.Time                       `json:"updatedAt"`
}

type GetActionStatusResponseAccount struct {
	Deposit GetActionStatusResponseStatus `json:"deposit"`
	Swap    GetActionStatusResponseStatus `json:"swap"`
}

type GetActionStatusResponseStatus struct {
	NumDifferentPools         int `json:"numDifferentPools"`
	NumDifferentPoolsToday    int `json:"numDifferentPoolsToday"`
	MaxNumDifferentPoolsToday int `json:"maxNumDifferentPoolsToday"`
}

type GetPoolsResponse PoolsCache

type GetPricesResponse PricesCache

type GetBannerResponse struct {
	Banner *GetBannerResponseBanner `json:"banner"`
}

type GetBannerResponseBanner struct {
	State    GetBannerResponseState `json:"state"`
	Text     string                 `json:"text"`
	URL      string                 `json:"url"`
	StartsAt time.Time              `json:"startsAt"`
	EndsAt   time.Time              `json:"endsAt"`
}

type GetBannerResponseState string

const (
	GetBannerResponseStateUpcoming = GetBannerResponseState("upcoming")
	GetBannerResponseStateStarted  = GetBannerResponseState("started")
)
