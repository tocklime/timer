let T = ./types.dhall
let map = https://prelude.dhall-lang.org/List/map

let xs = 
    [ "Star Jumps"
    , "Wall Sit"
    , "Push ups"
    , "Ab crunch"
    , "Chair step"
    , "Squat"
    , "Tricep dip"
    , "Plank"
    , "High knees"
    , "Lunge"
    , "Push up with rotation"
    , "Side plank 1"
    , "Side plank 2" ]
in
T.mkWorkout (toMap {
    set = T.set 10 (map Text (T.Work) (T.simple 30) xs)
}) (T.repeated 3 120 (T.ref "set"))
