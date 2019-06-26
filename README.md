# masyu solver
⌽ Solver for the puzzle game [Masyu](https://en.wikipedia.org/wiki/Masyu)

Though originally written in Python, the time to solve the puzzles was found to be unacceptably high.
A Rust version of the solver is currently under construction.
Both versions are available in this repo.

This project is a work in progress!

## Python Implementation Todos

At this point the Python version should be able to solve any board ...eventually.
It's ridiculously slow on larger boards: one 20x36 board I tested with took eight and a half hours to solve.

- The `solve_known_constraints` function is real slow on larger boards, which is bad because it's called a _ton_.
I think we could improve this time by removing circles from the set we check once their constraints are met.
- Should improve the order in which we select possibilities to expand.
Right now we always check cells in reading order, and short circuit if we learn any certainties one way or another.
I think if we prioritized already highly-constrained cells we might learn more certainties faster → constrain more cells sooner.
  - I tried naively randomizing the order in which we check cells, but that made the solve time of my test cases worse by like 1.5x.
- I really like the `ContradictionException` pattern, but I suspect this exception-based flow is silly slow.
Should probably replace with a return sentinel.
Honestly Rust's `Result` pattern is _exactly_ what I want for this in so many ways.
I'm an enormous fan.
Maybe I should double down and implement something like that for the Python version.


## Rust Implementation Todos

The Rust version can currently solve boards that only require one step of lookahead.
The release version of the code seems REAL fast which is promising, although it _has_ only been able to solve smaller, simpler boards so far.

- The `Lookahead` + `Possibility` tree structure needs the ability to store mutable parent references, which is the Rust equivalent of asking someone to carry you to Mars real quick.
Gotta figure out what the heck to do here.
- Should implement the one-time peephole optimizations from the Python version.
- The Python version does some extra work (for the modern definition of extra) to build off of existing paths between board states, which ideally cuts down on the time to reconstruct em.
The code to accomplish this is Rust-hostile, but the Rust version might still see a performance benefit from a different implementation that accomplishes the same thing.
Definitely worth benchmarking though: as mentioned, Rust is _already_ absurdly fast.
- Instead of emulating the Python trick of having all Board + Cell action functions return a new immutable Board, might be a nicer interface to have those functions just mutate the Board, then disallow mutability in the solver proper.
Downside is I think we have to clone _everything_ (or is it copy?) whenever we make a change, even if no change would occur.
Possibly worth looking into + comparing the performance difference.
