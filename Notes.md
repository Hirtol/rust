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

# Paper Notes

## Fluid Quotes: Metaprogramming across Abstraction Boundaries with Dependent Types [here](https://dl.acm.org/doi/pdf/10.1145/3425898.3426953)
Multi-stage programming systems is another avenue of allowing runtime macros (though at the cost of bundeling the compiler). Not possible with AoT compilers.
Fluid quotes require dependant types and the ability to expand the type checker with custom expansions, through macros or custom compiler APIs (Rust should be applicable here!)

The motivating example lists a situation where we need to get the method body of an opaque method call within a macro (e.g. macro!(2*preprocess(5)) ). This can't be done in most situations, this is where Fluid Quotes come in. This is similar(ish) to the compile time reflection initiative which died in Rustc earlier this year.

```scala
def getPreprocessor = {
    println("Initializing!")
    quote((in: Double) => in * in)
}

Then, we splice the function body with spliceCall. The
spliceCall macro provided by the fluid quotes API inserts
the body of the captured function at the call-site.

val preprocess = getPreprocessor
val derivative = differentiate((in: Double) => 2 * preprocess.spliceCall(in))

After spliceCall inlines the captured function, we have
a regular block that can be analyzed by differentiate:

val derivative = differentiate((in: Double) =>
    2 * { val x = in; x * x }
)

```
FluidQuotes does require that macros are expanded _during_ typechecking, so that the correct types can be generated and resolved. This would be difficult to do in Rustc! Essentially requires the Compile time reflection initiative, as there's currently no way to access the associated type of a trait.

The FluidQuote macro essentially transforms the given AST into an associated type (albeit stringified). It is transparent (aka, in Rust would implement Deref) to allow the users to use the type as if it wasn't transformed. This _requires_ the above (type check resolution during macro expansion) for their `splice` macro to work, as that accesses the associated type to retrieve the embedded AST.

When FluidQuotes refer to local variables it needs to introduce a closure to capture the environment to prevent hygiene contamination (e.g., later shadowing of a captured variable `num`). In principle this should be trivial to implement in Rustc, as one could rely on the existing Closure type generation (&local capture).
FluidQuotes identifies such to-be-captured variables by checking if a reference is not publicly accessible/defined within the quoted expression. 
It then generates 'Fwd' versions as variables within an associated Closure class. It only generates these 'Fwd' versions for non-public items (requires at-compile-time checking for publicness...) and rewrites such references to start from the `root` to keep macro hygiene (in case you shadow `math.max` for example).

The above does pose a problem in Rust that I think it assumes the existence of a GC. After all, there's no guarantee that one can copy a local variable (or reference it), the borrow checker is likely to get upset at that point. Although, having said that, maybe it isn't a big issue as one could implicitly `move` captured variables and emit a borrow-checking error if they're accessed _after_ the `quote` invocation.


Composing fluid quotes is not quite as flexible, as the types of the provided FluidQuotes would still be unknown (see the example in the paper). Because of
that such compositions only support generative macros, which do not inspect the passed ASTs (which is kind of the point!). It does, however, allow for elegant user-level macro(like) templates, where one could define an ordinary function taking `FluidQuotes` and have it, at compile time, expand to the appropriate code. E.g.:
```scala
def cappedLoop(condition: FluidQuote[Boolean],
                maxIterations: FluidQuote[Int])
                (thunk: FluidQuote[Unit]) = {
    quote {
        var itersLeft = maxIterations.splice
        while (itersLeft > 0 && condition.splice) {
        thunk.splice
        itersLeft -= 1
        }
    }
}
```

The FluidQuotes have a few restrictions, namely dynamic branches which return different `FluidQuotes` are impossible to remedy as they require type erasure, and thereby remove the information in the associated `Expr` type. This can have _some_ remedies, but they come down to a runtime evaluation of the branch, while splicing _all_ possible `FluidQuotes` thus ballooning code size.

### Applications
Fluid Quotes have some applications (such as making Streams more efficient) listed in the paper. An interesting note is that they compare their Stream efficiency improvements to the Rust equivalent (`Iterators`) and their Rust code doesn't compile : ). The fundamental notes on the limitations thereof are correct, however.

