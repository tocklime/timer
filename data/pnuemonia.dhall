let T = ./types.dhall in 
T.mkWorkout (toMap {
    inout = T.seq [T.simple 3 "Breathe in", T.simple 5 "Hold breath", T.simple 3 "Breathe out"],
    inouts = T.repeated 5 0 (T.ref "inout"),
    breathe = T.seq [T.ref "inouts", T.ref "incough"],
    incough = T.seq [T.simple 3 "Breathe in", T.simple 2 "Cough"]
}) (T.seq [T.ref "breathe", T.simple 600 "Lie on front with deep breaths"])
