
let SimpleWork : Type = 
    { name : Text, duration : Natural }

let Work : Type = < Ref : Text | Simple : SimpleWork >

let Set : Type = < Set : List Work | Repeat : { repeats : Natural, work : Work } >

let SetWithRests : Type = 
    { work : Set
    , rest: Natural
    } 

let KVP = {mapKey : Text, mapValue : SetWithRests}

let Workout : Type = 
    { definitions : List KVP 
    , top : Text
    }
let simple =
 \(dur : Natural) -> 
 \(name : Text) -> 
 	Work.Simple { name = name, duration = dur }

let repeated = 
 \(repeat : Natural) ->
 \(rest : Natural) -> 
 \(work : Work) -> 
 	{ rest = rest
    , work = Set.Repeat {repeats = repeat, work = work}
    }

let set = 
 \(rest : Natural) -> 
 \(work: List Work) ->
   { rest = rest
   , work = Set.Set work
   }

let seq = \(work : List Work) -> set 0 work

let mkWorkout = 
 \(defs : List KVP) ->
 \(top : Text) -> 
  { definitions = defs, top = top} 

let ref = Work.Ref

-- inlining this helpful method until dhall+wasm can do web requests :)
let map
    : ∀(a : Type) → ∀(b : Type) → (a → b) → List a → List b
    =   λ(a : Type)
      → λ(b : Type)
      → λ(f : a → b)
      → λ(xs : List a)
      → List/build
          b
          (   λ(list : Type)
            → λ(cons : b → list → list)
            → List/fold a xs list (λ(x : a) → cons (f x))
          )

in
