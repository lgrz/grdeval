# grdeval

A port of [gdeval.pl][gdeval] to Rust.

Currently only supports NDCG and ERR. Risk-sensitive evaluation is not yet
implemented.

The max judgment value required for ERR is derived from the supplied QRELS
file.

[gdeval]: https://github.com/trec-web/trec-web-2014
