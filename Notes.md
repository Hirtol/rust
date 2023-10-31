Conclusions 23-10-2023:
* After trying to change the caching to `TokenStream` instead of `AstFragment` I came to the following conclusion: 
Caching just the `TokenStream` or the `AstFragment` can't work due to the global
state inherent in Rustc. Somewhere during the conversion global state is modified (presumably
Span interning?) which causes the type-checking phase to throw out its cached results if the macro output was cached.
* The last alternative that I see now is to investigate the `proc_macro` server/client, and cache the _stable_ tokenstream
defined in the `proc_macro` crate. (this is separate from the rustc_ast `TokenStream`).
* If that isn't a proper option either then I'm afraid I'll have to give up this project, or use the results for _just_ `AstFragment` caching on 
macro expansion. This is an option as the main research question was whether it _could_ actually be faster (turns out, it can), even though overall compile times are destroyed due to the destruction of the query cache.

Conclusions 27-10-2023:
* The `proc_macro_bridge` is a mess to try and understand. Looking at discussions on the Rustc Zulip there's currently no
expert on this crate in the compiler team either :'). For some reason the client macro seems able to call interning methods on the server.
* It's rather unclear _where_ the `TokenStream` is actually initialised (and where the conversion code for `proc_macro::TokenStream` to `rustc_ast::TokenStream` actually resides?). It's a mess of macros, and global state references.
* This feels like a dead end due to the massive pain debugging this (reduced to `println!()` everywhere + recompile for 2 minutes) and presumably (due to interning related shenanigans) likely to futile anyway.

Conclusions 30-10-2023:
* Addendum to the above after several days of debugging: I think one _can_ cache the TokenStream, but the problem is the
hygiene context relevant in `Span` and `SpanData`. I assumed this context was created during parsing, but it seems like it's inherent to the `TokenStream` instead. This context reference is not deserialized by default, and thus must be done separately.
* Problem is, it's quite a complex task (Carried out by the `CacheEncoder`) at the moment. The `CacheEncoder` persists the 
query system cache to disk, alongside all the hygiene/expnIds/DefIds/etc. This leaves two options:
* One, I try to pull the query system back towards the front-end and piggyback off of the existing `CacheEncoder` infrastructure.
This is obviously the best option if this were to be something actually put into production, but a ludicrous amount of work, intractable within the scope of the Capita Selecta.
* Two, I try and copy the encoding/decoding from the `CacheEncoder`. Problem with this is that I still _don't_ understand what half the Ids/references are for (as there's no documentation justifying their existence!).
This would make mistakes incredibly likely and cause further debugging headaches due to poor tool support.
* In my opinion the best course of action is therefore either:
    1. Continue on with the existing `AstCache` as is and start benchmarking _just_ this section (as the trade-off/break even point was the main point of interest for me).
    This would require finding/creating the appropriate creates for comparative benchmarks to evaluate cost/improvement. We can then also start writing the paper on the existing Rustc query system, justification for Macro caching, and the difficulties in implementation thereof.
    2. If the former is not acceptable then the only other option is giving up and picking a different topic. Not fun or desired, but judging by the struggle to develop features within the current framework it would take a full quarter to implement the context encode/decoding properly anyway.
    At that point finding a project which gives something more 'academically satisfying' (a.k.a, something to write about in the paper/presentation) would be preferable. 3 Months of additional engineering effort isn't going to make it into the paper as it's not worth talking about in that respect. Hardly seems worth the effort then.