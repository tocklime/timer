let xs =
      [ "Star jumps"
      , "Wall sit"
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
      , "Side plank 2"
      ]

in  mkWorkout
      ( toMap
          { set = set 10 (map Text Work (simple 30) xs)
          , three_set = repeated 3 120 (ref "set")
          }
      )
      "set"
