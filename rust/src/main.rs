use std::collections::{HashMap, BTreeSet, VecDeque};
use std::fmt;
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
    fn opposite(&self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Right => Direction::Left,
            Direction::Left => Direction::Right,
        }
    }

    fn walk(&self, coord: Coord) -> Coord {
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

impl fmt::Display for Direction {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(formatter, "{}", self.to_string())
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

fn set_direction(cell_line: Rc<CellLine>, direction: Direction) -> Result<Rc<CellLine>, ContradictionException> {
    if cell_line.is_set.contains(&direction) {
        return Ok(cell_line);
    }
    if cell_line.cannot_set.contains(&direction) {
        return Err(ContradictionException {message: format!("Can't set {} on cell", direction)});
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
    return Ok(Rc::new(CellLine {is_set, cannot_set}))
}

fn disallow_direction(cell_line: Rc<CellLine>, direction: Direction) -> Result<Rc<CellLine>, ContradictionException> {
    if cell_line.cannot_set.contains(&direction) {
        return Ok(cell_line);
    }
    if cell_line.is_set.contains(&direction) {
        return Err(ContradictionException {message: format!("Can't disallow {} on cell", direction)});
    }

    let mut cannot_set = cell_line.cannot_set.clone();
    cannot_set.insert(direction);
    let is_set = cell_line.is_set.clone();
    return Ok(Rc::new(CellLine {is_set, cannot_set}))
}

fn get_through(cell_line: Rc<CellLine>) -> Result<Rc<CellLine>, ContradictionException> {
    let num_set = cell_line.is_set.len();
    if num_set == 2 {
        let mut ugh = cell_line.is_set.iter();
        let one = ugh.next().unwrap().clone();
        let other = ugh.next().unwrap().clone();
        if one.opposite() != other {
            return Err(ContradictionException {message: format!("{:?} is already bent!", cell_line)});
        }
        return Ok(cell_line);
    }
    if num_set == 1 {
        let mut ugh = cell_line.is_set.iter();
        let one = ugh.next().unwrap().clone();
        return set_direction(cell_line, one.opposite());
    }

    let num_cannot_set = cell_line.cannot_set.len();
    if num_cannot_set == 1 {
        let mut ugh = cell_line.cannot_set.iter();
        let one = ugh.next().unwrap().clone();
        let cannot_set = frozenset! {one, one.opposite()};
        return Ok(Rc::new(CellLine {is_set: Direction::all_but(&cannot_set), cannot_set}));
    }
    if num_cannot_set == 2 {
        let is_set = Direction::all_but(&cell_line.cannot_set);
        let mut ugh = is_set.iter();
        let one = ugh.next().unwrap().clone();
        let other = ugh.next().unwrap().clone();
        if one.opposite() != other {
            return Err(ContradictionException {message: format!("No straight path exists through {:?}", cell_line)});
        }
        return Ok(Rc::new(CellLine {is_set: is_set, cannot_set: cell_line.cannot_set.clone()}));
    }
    if num_cannot_set == 4 {
        return Err(ContradictionException {message: format!("{:?} must be blank", cell_line)});
    }
    assert!(num_cannot_set == 0, "expected no `cannot_set`, found {} ({:?})", num_cannot_set, cell_line.cannot_set);
    // We know nothing about this cell.
    return Ok(cell_line)
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
    return propagate_change(board, hashmap! {coord => new_cell})
}

fn set_through(board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    let old_cell = board.cell_lines.get(&coord).unwrap().clone();
    let new_cell = get_through(old_cell.clone())?;
    if new_cell == old_cell {
        return Ok(board)
    }
    return propagate_change(board, hashmap! {coord => new_cell})
}

fn chain_map_get<T: Eq + Hash, U>(maps: &[&HashMap<T, Rc<U>>], key: T) -> Option<Rc<U>> {
    for map in maps {
        if let Some(elem) = map.get(&key) {
            return Some(elem.clone())
        }
    }
    return None
}

fn propagate_change(board: Rc<Board>, mut changes: HashMap<Coord, Rc<CellLine>>) -> Result<Rc<Board>, ContradictionException> {
    let mut positions: VecDeque<Coord> = VecDeque::new();
    let mut ugh = changes.keys();
    positions.push_back(ugh.next().unwrap().clone());
    while let Some(coord) = positions.pop_front() {
        let cell = changes.get(&coord).unwrap().clone();
        for direction in cell.is_set.iter() {
            let mcoord = direction.walk(coord);
            let old_cell: Rc<CellLine> = chain_map_get(&[&changes, &board.cell_lines], mcoord).unwrap();
            let new_cell: Rc<CellLine> = set_direction(old_cell.clone(), direction.opposite())?;
            if new_cell == old_cell {continue}
            positions.push_back(mcoord.clone());
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

    // let cell_set = board.cell_lines.get(&coord).is_set
    // if cell_set.len() != 2 {
    //     return Ok(board);
    // }

    Ok(board)
}

fn apply_black(mut board: Rc<Board>, coord: Coord) -> Result<Rc<Board>, ContradictionException> {
    Ok(board)
}

fn solve_known_constraints(board: Rc<Board>) -> Result<Rc<Board>, ContradictionException> {
    let mut new_board = board;
    while {
        let old_board = new_board.clone();
        for (coord, circle) in new_board.clone().circles.iter() {
            new_board = match circle {
                CircleType::White => apply_white(new_board, *coord)?,
                CircleType::Black => apply_black(new_board, *coord)?,
            }
        }
        old_board != new_board
    } {}
    Ok(new_board)
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
    let lines = board_str.trim_end().split("\n").filter(|line| !line.starts_with("#")).collect::<Vec<_>>();
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
    return board_from_string(raw_data)
}

fn main() {
    let mut board = Rc::new(board_from_level("741".to_string()));
    board = solve_known_constraints(board).unwrap();
    // board = set_direction_on_board(board, Coord {x: 0, y: 0}, Direction::Right).expect("oof");
    // println!("{:?}", board.cell_lines.get(&Coord {x: 1, y: 0}));
    // println!("{:?}", board.cell_lines.get(&Coord {x: 2, y: 0}));
    // println!("{:?}", board.cell_lines.get(&Coord {x: 3, y: 0}));
    print_big_board(&board);
}
