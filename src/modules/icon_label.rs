use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{glib, glib::Properties};

glib::wrapper! {
    pub struct IconLabel(ObjectSubclass<imp::IconLabel>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl IconLabel {
    pub fn new(label: &str) -> Self {
        glib::Object::builder().property("label", label).build()
    }

    pub fn set_label(&self, label: &str) {
        self.set_property("label", label);
    }
}

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::IconLabel)]
    pub struct IconLabel {
        #[property(get, set = Self::set_label)]
        pub label_text: RefCell<String>,
        pub inner_label: gtk4::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IconLabel {
        const NAME: &'static str = "IconLabel";
        type Type = super::IconLabel;
        type ParentType = gtk4::Widget;
    }

    #[glib::derived_properties]
    impl ObjectImpl for IconLabel {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            self.inner_label.set_parent(&*obj);
            self.inner_label.set_overflow(gtk4::Overflow::Visible);
            obj.set_overflow(gtk4::Overflow::Visible);
        }

        fn dispose(&self) {
            self.inner_label.unparent();
        }
    }

    impl WidgetImpl for IconLabel {
        fn measure(&self, orientation: gtk4::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let (min, nat, min_baseline, nat_baseline) =
                self.inner_label.measure(orientation, for_size);

            if orientation == gtk4::Orientation::Horizontal {
                let layout = self.inner_label.layout();
                let (_, ink) = layout.pixel_extents();

                // The ink rectangle's width might be larger than the logical width
                let ink_width = ink.width() + ink.x().abs();

                let new_min = min.max(ink_width);
                let new_nat = nat.max(ink_width);

                return (new_min, new_nat, min_baseline, nat_baseline);
            }

            (min, nat, min_baseline, nat_baseline)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.inner_label.allocate(width, height, baseline, None);
        }
    }

    impl IconLabel {
        fn set_label(&self, value: String) {
            self.inner_label.set_label(&value);
            *self.label_text.borrow_mut() = value;
            self.obj().queue_resize();
        }
    }
}
