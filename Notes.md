Conclusions 23-10-2023:

-   After trying to change the caching to `TokenStream` instead of `AstFragment` I came to the following conclusion:
    Caching just the `TokenStream` or the `AstFragment` can't work due to the global
    state inherent in Rustc. Somewhere during the conversion global state is modified (presumably
    Span interning?) which causes the type-checking phase to throw out its cached results if the macro output was cached.
-   The last alternative that I see now is to investigate the `proc_macro` server/client, and cache the _stable_ tokenstream
    defined in the `proc_macro` crate. (this is separate from the rustc_ast `TokenStream`).
-   If that isn't a proper option either then I'm afraid I'll have to give up this project, or use the results for _just_ `AstFragment` caching on
    macro expansion. This is an option as the main research question was whether it _could_ actually be faster (turns out, it can), even though overall compile times are destroyed due to the destruction of the query cache.

Conclusions 27-10-2023:

-   The `proc_macro_bridge` is a mess to try and understand. Looking at discussions on the Rustc Zulip there's currently no
    expert on this crate in the compiler team either :'). For some reason the client macro seems able to call interning methods on the server.
-   It's rather unclear _where_ the `TokenStream` is actually initialised (and where the conversion code for `proc_macro::TokenStream` to `rustc_ast::TokenStream` actually resides?). It's a mess of macros, and global state references.
-   This feels like a dead end due to the massive pain debugging this (reduced to `println!()` everywhere + recompile for 2 minutes) and presumably (due to interning related shenanigans) likely to futile anyway.

Conclusions 30-10-2023:

-   Addendum to the above after several days of debugging: I think one _can_ cache the TokenStream, but the problem is the
    hygiene context relevant in `Span` and `SpanData`. I assumed this context was created during parsing, but it seems like it's inherent to the `TokenStream` instead. This context reference is not deserialized by default, and thus must be done separately.
-   Problem is, it's quite a complex task (Carried out by the `CacheEncoder`) at the moment. The `CacheEncoder` persists the
    query system cache to disk, alongside all the hygiene/expnIds/DefIds/etc. This leaves two options:
-   One, I try to pull the query system back towards the front-end and piggyback off of the existing `CacheEncoder` infrastructure.
    This is obviously the best option if this were to be something actually put into production, but a ludicrous amount of work, intractable within the scope of the Capita Selecta.
-   Two, I try and copy the encoding/decoding from the `CacheEncoder`. Problem with this is that I still _don't_ understand what half the Ids/references are for (as there's no documentation justifying their existence!).
    This would make mistakes incredibly likely and cause further debugging headaches due to poor tool support.
-   In my opinion the best course of action is therefore either:
    1. Continue on with the existing `AstCache` as is and start benchmarking _just_ this section (as the trade-off/break even point was the main point of interest for me).
       This would require finding/creating the appropriate creates for comparative benchmarks to evaluate cost/improvement. We can then also start writing the paper on the existing Rustc query system, justification for Macro caching, and the difficulties in implementation thereof.
    2. If the former is not acceptable then the only other option is giving up and picking a different topic. Not fun or desired, but judging by the struggle to develop features within the current framework it would take a full quarter to implement the context encode/decoding properly anyway.
       At that point finding a project which gives something more 'academically satisfying' (a.k.a, something to write about in the paper/presentation) would be preferable. 3 Months of additional engineering effort isn't going to make it into the paper as it's not worth talking about in that respect. Hardly seems worth the effort then.

# Paper Notes

## Fluid Quotes: Metaprogramming across Abstraction Boundaries with Dependent Types [here](https://dl.acm.org/doi/pdf/10.1145/3425898.3426953)

Multi-stage programming systems is another avenue of allowing runtime macros (though at the cost of bundeling the compiler). Not possible with AoT compilers.
Fluid quotes require dependent types and the ability to expand the type checker with custom expansions, through macros or custom compiler APIs (Rust should be applicable here!)

The motivating example lists a situation where we need to get the method body of an opaque method call within a macro (e.g. macro!(2\*preprocess(5)) ). This can't be done in most situations, this is where Fluid Quotes come in. This is similar(ish) to the compile time reflection initiative which died in Rustc earlier this year.

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

#### Additional Papers

Might be relevant based on references:

-   Macros that work together (offered a macro API that exposes compile-time information to enable macro cooperation)): http://dx.doi.org/10.1017/S0956796812000093

### Applications

Fluid Quotes have some applications (such as making Streams more efficient) listed in the paper. An interesting note is that they compare their Stream efficiency improvements to the Rust equivalent (`Iterators`) and their Rust code doesn't compile : ). The fundamental notes on the limitations thereof are correct, however.

## Virtual ADTs for Portable Metaprogramming [here](https://dl-acm-org.ezproxy2.utwente.nl/doi/pdf/10.1145/3475738.3480717)

Abstractly put, the paper talks about the difficulties of evolving existing ADTs (whether it be implementation detail, representation) when directly
relied on by users. It therefore proposes `Virtual ADTs` which offer the exact same interface as the concrete ADTs without relying on the implementation details.
In practice this means creating a Trait type which is implemented for your Concrete ADT with associated types referencing each individual element within the ADT (for application in RustLang this would require the 'Enum cases As Types initiative' to be implemented!). Note that isn't _technically_ required, e.g., one could instead implement the associated types as any random collection of types.

Any future reference to the ADT should go through the Virtual ADT (e.g., trait definition). This does seem to require type erasure to some extent (Can't specify the exact type of the associated types or the whole point of the abstraction is gone, so difficult to apply in Rust, _unless_ each individual associated type is in turn also an abstract type (e.g, see codeblock:)).

```rust
trait Peano {
    type Nat;
    // Declaring these types dependent would need to be a language extension itself.
    type Succ: Self::Nat;
    type Zero: Self::Nat;
}

trait Nat {
    fn plus(&self, other: Self) -> Self;
}

trait Succ {}

struct Impl;

impl Peano for Impl {
    type Nat = Box<dyn crate::Nat>;
    type Succ = Box<dyn crate::Succ>;

}
```

The remainder of the paper goes into detail on various extensions to this above definition (e.g, allowing pattern matching on the virtual ADT, or adding extension methods to the virtual ADT to mimic the original ADT's interface). It seems _very_ Scala specific in the problem it introduces and solves, don't think there's much transferable knowledge to Rust.

Pattern matching ala Scala definitely wouldn't be possible (so I guess that would need to be a language extension?).

## Generating C: Heterogeneous metaprogramming system description [here](https://pdf.sciencedirectassets.com/271600/1-s2.0-S0167642323X00069/1-s2.0-S0167642323000977/main.pdf)

The paper primarily talks about using OCaml (by writing OCaml source, compiling it, then running that executable) to generate _safe_, _correct_, and _performant_ C code. This resulting code should then be usable as regular C library code. It could also be compiled and dynamically linked into the generator.

One approach was apparently offshoring (potential reference: [here](https://link.springer.com/article/10.1007/s00354-007-0020-x)). Where essentially one translates the correct, higher level language (e.g., OCaml) into correct, low-level C. This paper improved upon this fundamental concept, outlining various challanges they encountered. This could be useful for the Rust compiler when looking at it from the perspective of benefitting from Rust memory safety guarantees, while not complicating a build system by avoiding the introduction of the Rust compiler, and instead generating C for use within the existing build system. If we explore this direction there's probably some Model Driven Engineering papers to use, as it's essentially the same idea as model translation (where we treat each language as the metamodel to translate between).

The challenges of directly generating C starts by concluding that any metaprogramming should _never_ represent other programs as Strings if you value your sanity, ala ATLAS. The obvious next approach is to therefore represent the data as an (sort of) AST and pretty print C as the last step. In comes off-shoring:

Off-shoring is usually done with some select subset of the higher level language which can (easily) be directly translated to C. Subsequently, generating valid OCaml (with say, macros) is in effect generating C through the use of offshoring. Turning homogenous metaprogramming to heterogenous. The former and later are the first and second premise of off-shoring, respectively. In the case of OCaml generating valid OCaml is done with MetaOCaml. (Question, would Rust Macros be the equivalent? Not entirely, as one is free to generate invalid TokenStreams which are only later marked as invalid after the parent Rustc type-checks the generated output. Then again, the `quote!` macro _does_ create well-formed Rust code -- similar to the brackets in MetaOcaml --, so maybe?). In fact, a Rust to C compiler already exists in a (limited) form: [here](https://github.com/thepowersgang/mrustc). Fun project idea: Implement a C code-gen as a compiler back-end. The prior project is a re-implementation of Rustc in C++.

```
Generating C via offshoring proceeds as:
1. implement the algorithm in OCaml
2. stage it – add staging annotations – and generate (possibly specialized) OCaml code (NOTE FOR SELF: don't quite get the necessity of this? Why not use 1. directly?)
(a) test the generated code
3. convert the generated OCaml code to C, saving it into a file
4. (a) compile and link the generated C code as ordinary C library code
(b) compile the generated C code and (dynamically) link into an OCaml program, via an FFI such as [20].
```

Addendum to the above: The use case for step 2. is nicely highlighted in page 5-6 of the paper! Essentially, it allows for generating a variety of different forms of the same function for different levels of optimisation. Good for experimenting, nothing you couldn't do with Rust macros as is.

-   (Bunch of examples to slowly build a re-usable OCaml `addv` generator function ala macros, cool, but skipping for now.) -

The offshorable subset of OCaml is the imperative part thereof. That is difficult to express in the OCaml type system (Could we do it in Rust? Probably not, would need extensions). Thus, one could pass generated OCaml using features not useable in the offshoring procedures. This doesn't invalidate the results, as it just throws an exception in such cases, but one would first always have to generate all that OCaml code first.

### Challenges

1. Type inference. Some of the generated OCaml code won't contain types, which must instead be inferred by the OCaml type checker. In the offshoring they use 2 separate IRs to derive the types. In a Rust equivalent one could use the HIR (as it will contain all desugared lifetimes/types), or just explicitly specify all types during staging.
2. Local Variables, namely that in OCaml variable declarations are expressions. Not as much a problem in Rust as (far as I am aware) declarations are statements. It's just that variable initialization _is_ an arbitrary expression. For OCaml they first translate to OffshoringIR (a third IR) which is statement oriented, which can then be translated directly to C99.
3. Extensibility, old off-shoring implementations weren't really extendable without recompiling the compiler. They addressed this by making their implementation a library which one can easily extend to implement support for OCaml unsupported types, external pointer types, etc, or call external functions. They create a separate OCaml module to allow one to bind external functions (no seeming way to specify calling convention??). This seems fragile to my eyes, but it might be I'm just not understanding their code correcly.
4. Control structures, For loops are rather restricted in OCaml, so they define a different macro function which allows them to emulate arbitrary step sizes. Rust wouldn't have quite the same problem, as for loops _are_ powerful enough for that, but are based on Ranges (which are iterators). Fully translating iterators would be difficult on a syntax level (may be possible from MIR onwards as everything is desugared/ready for LLVM generation at that point). Solving that would most likely be difficult, so one would maybe make a cut-off to only support basic Range syntax for the conversion from Rust to C.
5. No recursion. The off-shoring only off shores a single function, and OCaml doesn't have local function declarations (Rust does, so I suppose you could translate those?). For loops would therefore be preferred over recursion.
6. Pointer types. Essentially, the mutable variable semantics in OCaml can be difficult to translate in full to C without using more memory/variables. For this part see the actual section as it's too wordy to explain here. One solution is just restricting the off-shorable subset of OCaml. In Rust the same problems don't exist as reference types are almost entirely equal to C equivalents, including `mut` and `const`.

All in all the hetergenous meta language approach is best expressed in a language where composability is something that can easily be achieved. Need to evaluate how composable we could make Rust macros for expressing these patterns.

## Semantics-Preserving Inlining for Metaprogramming [here](https://sci-hub.se/https://doi.org/10.1145/3426426.3428486)

Unlikely to be relevant, as it introduces inlining for Scala, but the Rust compiler hints already exists (+ comptime evaluation).
The paper talks about inlining as a form of metaprogramming (after all, an inlined method _produces_ code at a call site). There is
a difference between _semantic_ and _syntactic_ inlining. In _semantic_ inlining the existing semantics of the program are preserved (See paper example).
In _syntactic_ the meaning could be changed due to, for example, type based method overloading.

Scala 3's `inline val x = 3` is essentially Rusts `const x: i32 = 3`, in that it replaces any instance with the right hand side directly.
With inline parameters (only possible for inline functions) one unlocks some metaprogramming, as one could force repetition of computation/side effects (by e.g., passing an inline closure, it is executed `n` times, but then possibly removed due to optimisations affored by that inlining)

When writing the section probably refer to: https://docs.scala-lang.org/scala3/guides/macros/inline.html

## A Survey of Metaprogramming Languages [here](https://sci-hub.se/https://dl.acm.org/doi/10.1145/3354584)

Could use this paper to classify Rust as a MetaLang into the same categories as the other languages in this paper. It is not currently included, so would be a (minor) novel contribution. Only a single paragraph of content, but hey, it's something!

In this taxonomy of metaprogramming languages the languages are classified. They identify three evaluation phases for metaprograms:

1. Before compilation, as a preprocessing step (C)
2. During compilation (Rust, C++ Templates (?))
3. During execution (Java Reflection, C# Harmony)

They also create classifications on the relation between the object language (to be generated language) and metalanguage (generating language):
Note that this classification is dependent on the selection of ObjLang. MetaML is, for example, category 2 when ObjLang is ML, but category 1 when ObjLang is MetaML.

1. MetaLang and ObjLang are indistinguishable (identical or subset) (Rust (Same constructs through same syntax. Common use of the quote! macro for actual code quoting, though. Splicing through nested quote! outputs))
2. MetaLang extending the ObjLang (MetaOCaml)
3. MetaLang being a different language (Heterogenous metaprogramming ala paper number 3.)

There is also the metaprogram source location as a point of classification:

1. In-source (Rust macro_rules! (context-unawaware), MetaOCaml, Lisp)
2. External source (Rust proc-macros (context-aware, though not possible according to their classification))

And lastly various methods for metaprogramming:

1. Macros (Rust, Lisp)
    - Two categories, lexical macros (language agnostic and operate on a lexical level, aka, token sequences. Rust(?, technically not, see section 3.1.2), C), and syntactic macros (which are aware of the language syntax and semantics, Rust macro-rules (CTMP), Lisp?)
2. Relfection (Java)
3. Metaobject Protocols (??)
4. Aspect Oriented Programming
5. Generative Programming (Automatically make a system based on a domain specific application specification, using pre-built (small) components)
    - C++ templates are an example of this actually!
    - Java annotation processors are another
6. Multistage Programming (Divide a program into levels of evaluation using staging annotations, allowing creation of delayed computations or specializations)
    - Closely related to partial evaluation
    - Can be seen as macro expansion. Macro systems use extra syntax for definitions and normal (function-like) syntax for invocations, while in MSP the extra syntax is needed at the call site to execute the returned delayed computation
    - MSLs are also categorized as homogeneous, when the metalanguage is the same as the object language, or heterogeneous, when the metalanguage and object language are different
    - MetaOCaml is an example of this approach, as could be seen in paper number 3!
    - C++ can be seen as a two stage language between template instantiation (first stage) and nontemplate code translation (second stage)

#### Additional Papers:

-   Inverse macro in Scala: https://dl.acm.org/doi/10.1145/2814204.2814213 (Initial inspection implies little Rust relevance, but could explore it more in-depth if we're short on content)
-   Gestalt (macro portability, looks related to VADT): https://infoscience.epfl.ch/record/231413?ln=en
-   Mython (Generative Programming, embedding C directly in Python code): https://dl-acm-org.ezproxy2.utwente.nl/doi/pdf/10.1145/1837513.1640141
-   A Practical Unification of Multi-stage Programming and Macros (multistage programming with quotes/splices and introduction thereof): https://dl-acm-org.ezproxy2.utwente.nl/doi/pdf/10.1145/3278122.3278139
-   Unifying Analytic and Statically-Typed Quasiquotes: https://dl.acm.org/doi/pdf/10.1145/3158101
