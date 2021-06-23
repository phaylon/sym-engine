
A simple experimental symbolic rule engine.

Mostly inteded as a platform for implementing bits of cognitive architectures for further
experimentation.

# Overview

The rules manipulate data in an object space. Object space is a directed cyclic graph.
Objects have attached attributes holding other objects, integers, floats, symbols, or tuples.
Values other than objects can not have attributes, but tuples can hold any values, including
objects.

Tuples are immutable segments of data. They can be used for partially matchable identification,
as encapsulation of unchangable data bits, or as messages between parts of the system.

Object rooting and garbage collection are available. Transactions can be used to encapsulate
changes to object space.

Each rule belongs to a specific system. Rules can be loaded into systems from files with
a basic rule language available, or built directly from via an API. Systems are then run
on an object space to apply rules.

Rule search is a basic search/apply loop. The only optimization currently done is a reordering
of the parts of the query based on a very simple cost analysis. This just ensures that things
like comparisons run a bit earlier if they can.

All of this is still changing a lot.