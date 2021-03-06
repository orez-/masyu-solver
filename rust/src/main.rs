use std::cell::RefCell;
use std::collections::{HashMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::hash::Hash;
use std::mem;
use std::rc::{Rc, Weak};


macro_rules! hashmap(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::new();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);

macro_rules! set(
    { $($key:expr),+ } => {
        {
            let mut m = ::std::collections::BTreeSet::new();
            $(
                m.insert($key);
            )+
            m
        }
    };
);

macro_rules! unpack1 {
    ($iter:expr) => {
        {
            assert!($iter.len() == 1, format!("Expected 1 value, found {}: {:?}", $iter.len(), $iter));
            let mut iter = $iter.iter();
            *iter.next().unwrap()
        }
    }
}

macro_rules! unpack2(
    ($iter:expr) => {
        {
            assert!($iter.len() == 2, format!("Expected 2 values, found {}: {:?}", $iter.len(), $iter));
            let mut iter = $iter.iter();
            (*iter.next().unwrap(), *iter.next().unwrap())
        }
    }
);

#[derive(Debug)]
#[derive(Clone, Copy)]
#[derive(Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    fn opposite(self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
        }
    }

    fn turn_left(self) -> Direction {
        match self {
            Direction::Up => Direction::Left,
            Direction::Down => Direction::Right,
            Direction::Right => Direction::Up,
            Direction::Left => Direction::Down,
        }
    }

    fn turn_right(self) -> Direction {
        match self {
            Direction::Up => Direction::Right,
            Direction::Down => Direction::Left,
            Direction::Right => Direction::Down,
            Direction::Left => Direction::Up,
        }
    }

    fn walk(self, coord: Coord) -> Coord {
        let (dx, dy) = match self {
            Direction::Up => (0, -1 as i8),
            Direction::Down => (0, 1),
            Direction::Right => (1, 0),
            Direction::Left => (-1 as i8, 0),
        };
        // My goodness I hate this.
        Coord {x: (coord.x as i8 + dx) as u8, y: (coord.y as i8 + dy) as u8}
    }

    fn all() -> BTreeSet<Direction> {
        set! {Direction::Up, Direction::Down, Direction::Right, Direction::Left}
    }

    fn all_but(except: &BTreeSet<Direction>) -> BTreeSet<Direction> {
        Direction::all().difference(except).cloned().collect()
    }
}

/// The attempted operation would result in a contradiction in board state!
#[derive(Debug)]
struct ContradictionException {message: String}

#[derive(Debug)]
struct LoopException (BTreeSet<Coord>);

impl LoopException {
    fn contains(&self, contains: &Coord) -> bool {
        self.0.contains(contains)
    }
}

#[derive(Debug)]
#[derive(Clone, Copy)]
#[derive(Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum CircleType {
    Black,
    White,
}

#[derive(Debug)]
#[derive(Clone, Copy)]
#[derive(Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct Coord {
    x: u8,
    y: u8,
}

#[derive(Debug)]
#[derive(Eq, PartialEq, Hash)]
pub struct CellLine {
    is_set: BTreeSet<Direction>,
    cannot_set: BTreeSet<Direction>,
}

impl CellLine {
    fn could_set(&self) -> BTreeSet<Direction> {
        Direction::all_but(&self.is_set).difference(&self.cannot_set).cloned().collect()
    }

    fn is_done(&self) -> bool {
        self.is_set.len() + self.cannot_set.len() == 4
    }

    fn other_out(&self, direction: Direction) -> Option<Direction> {
        if self.is_set.len() == 2 {
            let (one, other) = unpack2!(self.is_set);
            return Some(if direction == one {other}
            else if direction == other {one}
            else {panic!("missing direction");})
        }
        None
    }
}

fn set_direction(cell_line: Rc<CellLine>, direction: Direction) -> Result<Rc<CellLine>, ContradictionException> {
    if cell_line.is_set.contains(&direction) {
        return Ok(cell_line);
    }
    if cell_line.cannot_set.contains(&direction) {
        return Err(ContradictionException {message: format!("Can't set {:?} on cell", direction)});
    }

    let mut is_set = cell_line.is_set.clone();
    is_set.insert(direction);
    let mut cannot_set = cell_line.cannot_set.clone();

    if is_set.len() == 2 {
        cannot_set = Direction::all_but(&is_set)
    }
    else if cannot_set.len() == 2 {
        is_set = Direction::all_but(&cannot_set);
    }
    Ok(Rc::new(CellLine {is_set, cannot_set}))
}

fn disallow_direction(cell_line: Rc<CellLine>, direction: Direction) -> Result<Rc<CellLine>, ContradictionException> {
    if cell_line.cannot_set.contains(&direction) {
        return Ok(cell_line);
    }
    if cell_line.is_set.contains(&direction) {
        return Err(ContradictionException {message: format!("Can't disallow {:?} on cell", direction)});
    }

    let mut cannot_set = cell_line.cannot_set.clone();
    cannot_set.insert(direction);
    let mut is_set = cell_line.is_set.clone();

    // Need to make sure there _should_ be a line here.
    // Each cell does not necessarily contain a line!
    if cannot_set.len() == 2 && is_set.len() == 1 {
        is_set = Direction::all_but(&cannot_set);
    }
    else if cannot_set.len() == 3 {
        cannot_set = Direction::all();
    }
    Ok(Rc::new(CellLine {is_set, cannot_set}))
}

fn get_through(cell_line: Rc<CellLine>) -> Result<Rc<CellLine>, ContradictionException> {
    let num_set = cell_line.is_set.len();
    if num_set == 2 {
        let (one, other) = unpack2!(cell_line.is_set);
        if one.opposite() != other {
            return Err(ContradictionException {message: format!("{:?} is already bent!", cell_line)});
        }
        return Ok(cell_line);
    }
    if num_set == 1 {
        let one = unpack1!(cell_line.is_set);
        return set_direction(cell_line, one.opposite());
    }

    let num_cannot_set = cell_line.cannot_set.len();
    if num_cannot_set == 1 {
        let one = unpack1!(cell_line.cannot_set);
        let cannot_set = set! {one, one.opposite()};
        return Ok(Rc::new(CellLine {is_set: Direction::all_but(&cannot_set), cannot_set}));
    }
    if num_cannot_set == 2 {
        let is_set = Direction::all_but(&cell_line.cannot_set);
        let (one, other) = unpack2!(is_set);
        if one.opposite() != other {
            return Err(ContradictionException {message: format!("No straight path exists through {:?}", cell_line)});
        }
        return Ok(Rc::new(CellLine {is_set, cannot_set: cell_line.cannot_set.clone()}));
    }
    if num_cannot_set == 4 {
        return Err(ContradictionException {message: format!("{:?} must be blank", cell_line)});
    }
    assert!(num_cannot_set == 0, "expected no `cannot_set`, found {} ({:?})", num_cannot_set, cell_line.cannot_set);
    // We know nothing about this cell.
    Ok(cell_line)
}

fn get_bent(cell_line: Rc<CellLine>) -> Result<Rc<CellLine>, ContradictionException> {  // 💁‍♀
    let num_set = cell_line.is_set.len();
    if num_set == 2 {
        let (one, other) = unpack2!(cell_line.is_set);
        if one.opposite() == other {
            return Err(ContradictionException {message: format!("{:?} is already straight-through!", cell_line)});
        }
        return Ok(cell_line);
    }
    if num_set == 1 {
        let one = unpack1!(cell_line.is_set);
        return disallow_direction(cell_line, one.opposite());
    }

    let num_cannot_set = cell_line.cannot_set.len();
    if num_cannot_set == 1 {
        let one = unpack1!(cell_line.cannot_set);
        return set_direction(cell_line, one.opposite());
    }
    if num_cannot_set == 2 {
        let is_set = Direction::all_but(&cell_line.cannot_set);
        let (one, other) = unpack2!(is_set);
        if one.opposite() == other {
            return Err(ContradictionException {message: format!("No bent path exists through {:?}", cell_line)});
        }
        return Ok(Rc::new(CellLine {is_set, cannot_set: cell_line.cannot_set.clone()}));
    }
    if num_cannot_set == 4 {
        return Err(ContradictionException{message: format!("{:?} must be blank", cell_line)});
    }

    Ok(cell_line)
}

#[derive(Debug)]
struct LineSegment {
    start: Coord,
    start_direction: Direction,
    end: Coord,
    end_direction: Direction,
    contains: BTreeSet<Coord>,
}

fn discover_line_segments(cell_lines: &HashMap<Coord, Rc<CellLine>>, mut seen: BTreeSet<Coord>) -> Result<Vec<Rc<LineSegment>>, LoopException> {
    let mut line_segment = Vec::new();
    for (coord, cell) in cell_lines {
        if seen.contains(&coord) || cell.is_set.is_empty() {
            continue;
        }

        let mut segment = set! {*coord};
        // Backward, and check for closed loop.
        // If there is no backward we're already at the start

        let mut forward_dir: Direction;
        let mut back_dir: Direction;
        let mut start = *coord;
        let mut end = *coord;
        if cell.is_set.len() == 1 {
            back_dir = unpack1!(cell.is_set);
            forward_dir = back_dir;
        }
        else {
            let (dumb, stupid) = unpack2!(cell.is_set);
            forward_dir = dumb;
            back_dir = stupid;

            for (start_local, back_dir_local) in cell_path(*coord, back_dir, &cell_lines) {
                start = start_local;
                back_dir = back_dir_local;
                if segment.contains(&start) {
                    // We've got a closed loop! This is exceptional!
                    // We're definitely either done or wrong.
                    // Either way, stop what you're doing and say something!
                    return Err(LoopException(segment));
                }
                segment.insert(start);
            }
        }

        for (end_local, forward_dir_local) in cell_path(*coord, forward_dir, &cell_lines) {
            end = end_local;
            forward_dir = forward_dir_local;
            segment.insert(end);
        }

        seen.append(&mut segment.clone());
        line_segment.push(
            Rc::new(LineSegment {
                start,
                start_direction: back_dir,
                end,
                end_direction: forward_dir,
                contains: segment,
            })
        );
    }
    Ok(line_segment)
}

struct CellPath<'a> {
    coord: Coord,
    direction: Option<Direction>,
    cell_lines: &'a HashMap<Coord, Rc<CellLine>>,
}

impl <'a> Iterator for CellPath<'a> {
    type Item = (Coord, Direction);
    fn next(&mut self) -> Option<(Coord, Direction)> {
        let mut direction = self.direction?;
        self.coord = direction.walk(self.coord);
        direction = direction.opposite();
        // yield coord, direction
        let cell = self.cell_lines.get(&self.coord).unwrap();
        self.direction = cell.other_out(direction);
        Some((self.coord, direction))
    }
}

fn cell_path(coord: Coord, direction: Direction, cell_lines: &HashMap<Coord, Rc<CellLine>>) -> CellPath {
    CellPath {coord, direction: Some(direction), cell_lines}
}

struct Board {
    width: u8,
    height: u8,
    // XXX since the lifetime of `circles` is Very Known (it's the lifetime of the solve),
    // maybe this should/could be a reference instead of Rc'd
    circles: Rc<HashMap<Coord, CircleType>>,
    cell_lines: HashMap<Coord, Rc<CellLine>>,
    line_segments: Vec<Rc<LineSegment>>,
    solved: bool,
}

impl PartialEq for Board {
    fn eq(&self, rhs: &Self) -> bool {
        // Technically we should check width, height, and circles to be sure,
        // but realistically we're never going to compare two different puzzles
        self.cell_lines == rhs.cell_lines
    }
}

impl Eq for Board {}

impl std::fmt::Debug for Board {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "Board")
    }
}

fn set_direction_on_board(board: Rc<Board>, coord: Coord, direction: Direction) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = set_direction(old_cell.clone(), direction)?;
    if new_cell == old_cell {
        return Ok(board)
    }
    propagate_change(board, hashmap! {coord => new_cell})
}

fn disallow_direction_on_board(board: Rc<Board>, coord: Coord, direction: Direction) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = disallow_direction(old_cell.clone(), direction)?;
    if new_cell == old_cell {
        return Ok(board)
    }
    propagate_change(board, hashmap! {coord => new_cell})
}

fn set_through(board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = get_through(old_cell.clone())?;
    if new_cell == old_cell {
        return Ok(board)
    }
    propagate_change(board, hashmap! {coord => new_cell})
}

fn set_bent(board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = get_bent(old_cell.clone())?;
    if new_cell == old_cell {
        return Ok(board)
    }
    propagate_change(board, hashmap! {coord => new_cell})
}

fn chain_map_get<T: Eq + Hash, U>(maps: &[&HashMap<T, Rc<U>>], key: T) -> Option<Rc<U>> {
    for map in maps {
        if let Some(elem) = map.get(&key) {
            return Some(elem.clone())
        }
    }
    None
}

fn propagate_change(board: Rc<Board>, mut changes: HashMap<Coord, Rc<CellLine>>) -> Result<Rc<Board>, ContradictionException> {
    let mut solved = false;
    let mut positions: VecDeque<Coord> = VecDeque::new();
    positions.push_back(changes.keys().next().unwrap().clone());
    while let Some(coord) = positions.pop_front() {
        let cell = changes.get(&coord).unwrap().clone();
        for direction in cell.is_set.iter() {
            let mcoord = direction.walk(coord);
            let old_cell: Rc<CellLine> = chain_map_get(&[&changes, &board.cell_lines], mcoord).unwrap();
            let new_cell: Rc<CellLine> = set_direction(old_cell.clone(), direction.opposite())?;
            if new_cell == old_cell {continue}
            positions.push_back(mcoord);
            changes.insert(mcoord, new_cell);
        }

        for direction in cell.cannot_set.iter() {
            let mcoord = direction.walk(coord);
            if let Some(old_cell) = chain_map_get(&[&changes, &board.cell_lines], mcoord) {
                let new_cell = disallow_direction(old_cell.clone(), direction.opposite())?;
                if new_cell == old_cell {continue}
                positions.push_back(mcoord);
                changes.insert(mcoord, new_cell);
            }
        }
    }
    let cell_lines = board.cell_lines.clone().into_iter().chain(changes).collect();

    let line_segments = match discover_line_segments(&cell_lines, BTreeSet::new()) {
        Ok(segments) => segments,
        Err(loop_path) => {
            if !board.circles.keys().all(|coord| loop_path.contains(coord)) {
                return Err(ContradictionException {message: "Closed loop does not contain all circles".to_string()});
            }
            // Otherwise, this is a victory!
            solved = true;
            Vec::new()
        }
    };

    Ok(Rc::new(Board {
        width: board.width,
        height: board.height,
        circles: board.circles.clone(),
        cell_lines,
        line_segments,
        solved,
    }))
    // evolve(board, changes)
}

// fn evolve(board: Rc<Board>, cell_lines: HashMap<Coord, Rc<CellLine>>) -> Result<Rc<Board>, ContradictionException> {
//     return Ok(board);
// }

fn apply_white(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    board = set_through(board, coord)?;

    let cell_set = &board.cell_lines.get(&coord).unwrap().is_set;
    if cell_set.len() != 2 {
        return Ok(board);
    }

    let (left, right) = unpack2!(cell_set);
    let left_coord = left.walk(coord);
    let bend_left = set_bent(board.clone(), left_coord);
    let right_coord = right.walk(coord);
    let bend_right = set_bent(board.clone(), right_coord);

    if bend_left.is_err() && bend_right.is_err() {
        return Err(ContradictionException {message: format!("Cannot bend either end of the white circle at {:?}", coord)})
    }

    if bend_left.is_ok() && bend_right.is_ok() {
        // We don't really know anything: either could bend.
        return Ok(board);
    }

    // We know something at this point though: only one may bend!
    // Bend that one!!
    bend_left.or(bend_right)
}

fn apply_black(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    board = set_bent(board, coord)?;
    let dumb_ref = board.clone();  // rust doesn't let me inline this! wtf!
    let cell = dumb_ref.cell_lines.get(&coord).unwrap();

    // extend existing lines
    for direction in cell.is_set.iter() {
        board = set_through(board, direction.walk(coord))?;
    }

    if cell.is_done() {
        return Ok(board);
    }

    let mut could_dirs = BTreeSet::new();
    for direction in cell.could_set() {
        if set_black_leg(board.clone(), coord, direction).is_ok() {
            could_dirs.insert(direction);
        }
    }

    for direction in could_dirs.iter() {
        if !could_dirs.contains(&direction.opposite()) {
            board = set_black_leg(board, coord, *direction)?;
        }
    }

    Ok(board)
}

fn set_black_leg(mut board: Rc<Board>, coord: Coord, direction: Direction) -> Result<Rc<Board>, ContradictionException> {
    board = set_direction_on_board(board, coord, direction)?;
    set_through(board, direction.walk(coord))
}

fn solve_known_constraints(mut board: Rc<Board>) -> Result<Rc<Board>, ContradictionException> {
    while {
        let old_board = board.clone();
        for (coord, circle) in board.clone().circles.iter() {
            board = match circle {
                CircleType::White => apply_white(board, *coord)?,
                CircleType::Black => apply_black(board, *coord)?,
            }
        }
        old_board != board
    } {}
    Ok(board)
}

/// A board and all of its potential next states.
/// If the board's next states are unexplored, None is kept instead.
#[derive(Debug)]
struct Lookahead {
    board: Rc<Board>,
    parent: Option<Weak<RefCell<PossibilityPair>>>,
    possibilities: Option<Vec<Rc<RefCell<PossibilityPair>>>>,
}

impl Lookahead {
    fn new(board: Rc<Board>) -> Self {
        Lookahead {board, parent: None, possibilities: None}
    }
}


fn explore(root_lookahead: &Rc<RefCell<Lookahead>>) -> Result<bool, ContradictionException> {
    let mut queue: VecDeque<Rc<RefCell<Lookahead>>> = VecDeque::new();
    queue.push_back(root_lookahead.clone());
    while let Some(lookahead) = queue.pop_front() {
        // Need to explicitly drop this borrow in the `else` case so we can
        // borrow_mut in `expand`. Not sure why the borrow would persist
        // across to the `else` case but I assume the people who wrote Rust
        // are smarter than me ᖍ(•⟝•)ᖌ
        let lookahead_borrow = lookahead.borrow();
        if let Some(ref possibilities) = lookahead_borrow.possibilities {
            for pos in possibilities {
                queue.push_back(pos.borrow().yes.clone());
                queue.push_back(pos.borrow().no.clone());
            }
        }
        else {
            // Drop dem refs (see above)
            mem::drop(queue);
            mem::drop(lookahead_borrow);
            expand(&lookahead)?;
            return Ok(true);
        };
    }
    Ok(false)
}

fn expand(lookahead: &Rc<RefCell<Lookahead>>) -> Result<(), ContradictionException> {
    assert!(lookahead.borrow().possibilities.is_none());
    match get_possibility_list(lookahead) {
        LookaheadOutcome::Certainty(new_board) => {lookahead.borrow_mut().board = new_board},
        LookaheadOutcome::Possibilities(new_poss) => {lookahead.borrow_mut().possibilities = Some(new_poss)},
        LookaheadOutcome::Contradiction => {
            // Contradiction is BIG.
            // Promote my sibling Lookahead to our PossibilityPair's parent Lookahead
            // That is:

            // Lookahead-root:
            //   - PossibilityPair-1:
            //       Lookahead-1-yes: CONTRADICTION
            //       Lookahead-2-no: unexplored

            // Becomes:

            // Lookahead-2-no: unexplored
            let sibling = get_sibling(lookahead)?;
            let grandparent = Weak::upgrade(&Weak::upgrade(&lookahead.borrow().parent.clone().unwrap()).unwrap().borrow().parent).unwrap();
            sibling.borrow_mut().parent = (&*grandparent).borrow().parent.clone();

            if let Some(ref possibilities) = sibling.borrow().possibilities {
                for pos in possibilities {
                    pos.borrow_mut().parent = Rc::downgrade(&grandparent);
                }
            }
            // We're about to pull a reverse Get Out: putting `sibling`'s soul into `grandparent`'s body.
            // Before we can get at the `sibling`'s soul we need to ensure we have the only strong
            // reference to it, but the grandparent currently still refers to it. Fortunately since
            // we're about to burn it all down anyway we can nuke that reference by burning down
            // `grandparent`'s children explicitly.
            // Note: the preceding comment contained spoilers for the movie Get Out.
            grandparent.borrow_mut().possibilities = None;
            grandparent.replace(Rc::try_unwrap(sibling).expect("dammit we got two Rc references").into_inner());
        },
    }
    Ok(())
}

fn get_sibling(lookahead: &Rc<RefCell<Lookahead>>) -> Result<Rc<RefCell<Lookahead>>, ContradictionException> {
    match lookahead.borrow().parent.clone() {
        Some(parent_wrapper_hell) => {
            let parent = Weak::upgrade(&parent_wrapper_hell).unwrap();
            if Rc::ptr_eq(lookahead, &parent.borrow().yes) {
                Ok(parent.borrow().no.clone())
            }
            else if Rc::ptr_eq(lookahead, &parent.borrow().no) {
                Ok(parent.borrow().yes.clone())
            }
            else {
                panic!("Lookahead's parent does not have it as a child. The heck??");
            }
        },
        None => Err(ContradictionException {message: "root lookahead encountered contradiction".to_string()}),
    }
}

fn get_possibility_list(lookahead: &Rc<RefCell<Lookahead>>) -> LookaheadOutcome {
    let board = &lookahead.borrow().board;
    let mut possibilities = Vec::new();
    let mask = set! {Direction::Right, Direction::Down};
    for (&coord, cell) in board.cell_lines.iter() {
        for &direction in cell.could_set().intersection(&mask) {
            match (
                set_direction_on_board(board.clone(), coord, direction).and_then(solve_known_constraints),
                disallow_direction_on_board(board.clone(), coord, direction).and_then(solve_known_constraints),
            ) {
                (Err(_), Err(_)) => {return LookaheadOutcome::Contradiction},
                (Ok(yes), Ok(no)) => {possibilities.push(PossibilityPair::new(yes, no, &lookahead))},
                (Ok(yes), _) => {return LookaheadOutcome::Certainty(yes)},
                (_, Ok(no)) => {return LookaheadOutcome::Certainty(no)},
            }
        }
    }
    LookaheadOutcome::Possibilities(possibilities)
}

/// Observe a given board, coordinate, and direction.
/// Extrapolate the state of the board if that coordinate and direction
/// had a line and store the result in `yes`. Similarly, extrapolate if
/// it definitely _did not_ have a line and store the result in `no`.
///
/// Note that the original board and the exact values of the coordinate
/// and direction are irrelevant, and are not kept in this data structure.
#[derive(Debug)]
struct PossibilityPair {
    yes: Rc<RefCell<Lookahead>>,
    no: Rc<RefCell<Lookahead>>,
    parent: Weak<RefCell<Lookahead>>,
}

impl PossibilityPair {
    fn new(yes_board: Rc<Board>, no_board: Rc<Board>, parent: &Rc<RefCell<Lookahead>>) -> Rc<RefCell<Self>> {
        // Need to do a goofy dance here to get the pair to point to the lookaheads, and vice versa
        let pair = Rc::new(RefCell::new(PossibilityPair {
            yes: Rc::new(RefCell::new(Lookahead::new(yes_board))),
            no: Rc::new(RefCell::new(Lookahead::new(no_board))),
            parent: Rc::downgrade(parent),
        }));
        pair.borrow().yes.borrow_mut().parent = Some(Rc::downgrade(&pair));
        pair.borrow().no.borrow_mut().parent = Some(Rc::downgrade(&pair));
        pair
    }
}

enum LookaheadOutcome {
    Possibilities(Vec<Rc<RefCell<PossibilityPair>>>),
    Certainty(Rc<Board>),
    Contradiction,
}

fn _extract_board(lookahead: Rc<RefCell<Lookahead>>) -> Rc<Board> {
    Rc::try_unwrap(lookahead).unwrap().into_inner().board
}


fn solve_lookaheads(board: Rc<Board>) -> Result<Rc<Board>, ContradictionException> {
    let root = Rc::new(RefCell::new(Lookahead::new(solve_known_constraints(board)?)));
    loop {
        if !explore(&root)? {
            println!("Stuck!");
            return Ok(_extract_board(root))
        }
        if root.borrow().board.solved {
            return Ok(_extract_board(root))
        }
        if cfg!(debug_assertions) {
            print_big_board(&*root.borrow().board);
        }
    }
}

fn solve_three_consecutive_whites(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    // ooo
    let right1 = Direction::Right.walk(coord);
    let right2 = Direction::Right.walk(right1);
    let down1 = Direction::Down.walk(coord);
    let down2 = Direction::Down.walk(down1);
    let white = Some(&CircleType::White);
    if board.circles.get(&right1) == white && board.circles.get(&right2) == white {
        board = set_direction_on_board(board, coord, Direction::Up)?;
        board = set_through(board, coord)?;
        board = set_through(board, right1)?;
        board = set_through(board, right2)?;
    }
    else if board.circles.get(&down1) == white && board.circles.get(&down2) == white {
        board = set_direction_on_board(board, coord, Direction::Right)?;
        board = set_through(board, coord)?;
        board = set_through(board, down1)?;
        board = set_through(board, down2)?;
    }
    Ok(board)
}

fn solve_overlong_leg(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    // ●?oo
    for direction in Direction::all() {
        let first_white = direction.walk(direction.walk(coord));
        let next_white = direction.walk(first_white);
        let white = Some(&CircleType::White);
        if board.circles.get(&first_white) == white && board.circles.get(&next_white) == white {
            board = set_black_leg(board, coord, direction.opposite())?;
        }
    }
    Ok(board)
}

fn solve_adjacent_blacks(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    // ●●
    let down = Direction::Down.walk(coord);
    let right = Direction::Right.walk(coord);
    let black = Some(&CircleType::Black);
    if board.circles.get(&right) == black {
        board = set_black_leg(board, coord, Direction::Left)?;
        board = set_black_leg(board, right, Direction::Right)?;
    }
    if board.circles.get(&down) == black {
        board = set_black_leg(board, coord, Direction::Up)?;
        board = set_black_leg(board, down, Direction::Down)?;
    }
    Ok(board)
}

fn solve_wingman_black(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    // ?●?
    // o?o
    let white = Some(&CircleType::White);
    for direction in Direction::all() {
        let ahead = direction.walk(coord);
        let left = direction.turn_left().walk(ahead);
        let right = direction.turn_right().walk(ahead);
        if board.circles.get(&left) == white && board.circles.get(&right) == white {
            board = set_black_leg(board, coord, direction.opposite())?;
        }
    }
    Ok(board)
}

fn solve_initial_patterns(mut board: Rc<Board>) -> Result<Rc<Board>, ContradictionException> {
    for (coord, color) in board.clone().circles.iter() {
        match color {
            CircleType::White => {
                board = solve_three_consecutive_whites(board, *coord)?;
            },
            CircleType::Black => {
                board = solve_overlong_leg(board, *coord)?;
                board = solve_adjacent_blacks(board, *coord)?;
                board = solve_wingman_black(board, *coord)?;
            },
        }
    }
    Ok(board)
}

fn print_big_board(board: &Board) {
    let inner_cell_line = hashmap! {
        set! {Direction::Down, Direction::Up} => "│",
        set! {Direction::Left, Direction::Right} => "─",
        set! {Direction::Left, Direction::Down} => "┐",
        set! {Direction::Left, Direction::Up} => "┘",
        set! {Direction::Right, Direction::Up} => "└",
        set! {Direction::Right, Direction::Down} => "┌"
    };
    let gray = "\x1b[38;5;8m";
    let clear = "\x1b[0m";
    let mut board_str = String::new();
    board_str.push_str(gray);
    board_str.push_str("┌");
    board_str.push_str(&vec!["─"; board.width as usize].join("┬"));
    board_str.push_str("┐\n");

    for row in 0..board.height {
        board_str.push_str("│");
        board_str.push_str(clear);
        for col in 0..board.width {
            let coord = Coord {x: col, y: row};
            let cell = board.cell_lines.get(&coord).unwrap();
            board_str.push_str(match board.circles.get(&coord) {
                Some(CircleType::Black) => "●",
                Some(CircleType::White) => "o",
                None => {
                    let cell = board.cell_lines.get(&coord).expect("missing cell line");
                    inner_cell_line.get(&cell.is_set).unwrap_or(&" ")
                }
            });
            if cell.is_set.contains(&Direction::Right) {
                board_str.push_str("─")
            }
            else {
                board_str.push_str(gray);
                board_str.push_str("│");
                board_str.push_str(clear);
            }
        }
        board_str.push_str(gray);
        if row == board.height - 1 {
            board_str.push_str("\n└");
            board_str.push_str(&vec!["─"; board.width as usize].join("┴"));
            board_str.push_str("┘");
            board_str.push_str(clear);
        }
        else {
            board_str.push_str("\n├");
            for col in 0..board.width {
                let coord = Coord {x: col, y: row};
                let cell = board.cell_lines.get(&coord).unwrap();
                if cell.is_set.contains(&Direction::Down) {
                    board_str.push_str(clear);
                    board_str.push_str("│");
                    board_str.push_str(gray);
                }
                else {board_str.push_str("─");}
                board_str.push_str(if col == board.width - 1 {"┤"} else {"┼"});
            }
        }
        board_str.push_str("\n");
    }

    println!("{}", board_str);
}

fn board_from_string(board_str: String) -> Board {
    let mut circles = HashMap::new();
    let lines = board_str.trim_end().split('\n').filter(|line| !line.starts_with('#')).collect::<Vec<_>>();
    for (y, line) in lines.iter().enumerate() {
        for (x, elem) in line.chars().enumerate() {
            match elem {
                'o' => {circles.insert(Coord{x: x as u8, y: y as u8}, CircleType::White);},
                '●' => {circles.insert(Coord{x: x as u8, y: y as u8}, CircleType::Black);},
                '.' => (),
                letter => panic!(format!("Unexpected character {}", letter))
            }
        }
    }

    let width = lines[0].chars().count() as u8;
    let height = lines.len() as u8;

    let mut cell_lines = HashMap::new();
    for y in 0..height {
        for x in 0..width {
            let mut edges = BTreeSet::new();
            if x == 0 {edges.insert(Direction::Left);}
            if y == 0 {edges.insert(Direction::Up);}
            if y == height - 1 {edges.insert(Direction::Down);}
            if x == width - 1 {edges.insert(Direction::Right);}
            let cell_line = CellLine {is_set: BTreeSet::new(), cannot_set: edges};
            cell_lines.insert(Coord{x, y}, Rc::new(cell_line));
        }
    }

    Board {width, height, circles: Rc::new(circles), cell_lines, line_segments: Vec::new(), solved: false}
}

fn board_from_level(level_name: String) -> Board {
    let raw_data = fs::read_to_string(format!("../levels/{}.masyu", level_name)).expect("Unable to read file");
    board_from_string(raw_data)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut board = Rc::new(board_from_level(args[1].to_string()));
    board = solve_initial_patterns(board).unwrap();
    board = solve_lookaheads(board).unwrap();
    print_big_board(&board);
}
