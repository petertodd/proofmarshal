use quote::quote;
use synstructure::decl_derive;


mod commit;
use self::commit::*;
decl_derive!([Commit, attributes(foo)] => derive_commit);

mod prune;
use self::prune::*;
decl_derive!([Prune, attributes(foo)] => derive_prune);
