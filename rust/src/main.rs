use std::collections::{HashMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::hash::Hash;
use std::rc::Rc;


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

macro_rules! frozenset(
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
            let mut iter = $iter.iter();
            *iter.next().unwrap()
        }
    }
}

macro_rules! unpack2(
    ($iter:expr) => {
        {
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
        frozenset! {Direction::Up, Direction::Down, Direction::Right, Direction::Left}
    }

    fn all_but(except: &BTreeSet<Direction>) -> BTreeSet<Direction> {
        Direction::all().difference(except).cloned().collect()
    }
}

/// The attempted operation would result in a contradiction in board state!
#[derive(Debug)]
struct ContradictionException {message: String}

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
        let cannot_set = frozenset! {one, one.opposite()};
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
#[derive(Eq, PartialEq)]
pub struct Board {
    width: u8,
    height: u8,
    circles: Rc<HashMap<Coord, CircleType>>,
    cell_lines: HashMap<Coord, Rc<CellLine>>,
}

fn set_direction_on_board(board: Rc<Board>, coord: Coord, direction: Direction) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = set_direction(old_cell.clone(), direction)?;
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

    Ok(Rc::new(Board {
        width: board.width,
        height: board.height,
        circles: board.circles.clone(),
        cell_lines,
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

fn print_big_board(board: &Board) {
    let inner_cell_line = hashmap! {
        frozenset! {Direction::Down, Direction::Up} => "│",
        frozenset! {Direction::Left, Direction::Right} => "─",
        frozenset! {Direction::Left, Direction::Down} => "┐",
        frozenset! {Direction::Left, Direction::Up} => "┘",
        frozenset! {Direction::Right, Direction::Up} => "└",
        frozenset! {Direction::Right, Direction::Down} => "┌"
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

    let width = lines[0].len() as u8;
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

    Board {width, height, circles: Rc::new(circles), cell_lines}
}

fn board_from_level(level_name: String) -> Board {
    let raw_data = fs::read_to_string(format!("../levels/{}.masyu", level_name)).expect("Unable to read file");
    board_from_string(raw_data)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut board = Rc::new(board_from_level(args[1].to_string()));
    board = solve_known_constraints(board).unwrap();
    print_big_board(&board);
}
