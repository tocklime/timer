--let T = ./types.dhall in

mkWorkout (toMap 
    { set = repeated 10 30 (simple 30 "Work")
    , two_sets = repeated 2 120 (ref "set")
    , all = seq [simple 300 "Warmup", ref "two_sets", simple 300 "Stretches"]
    }) "all"
